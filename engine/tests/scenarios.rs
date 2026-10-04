//! The YAML scenario suite (`tests/scenarios/`, `docs/scenarios.md`).
//!
//! Every scenario is its own test, named `<dir>::<file>::<scenario>`
//! (`cargo test --test scenarios -- tavern::joyous::`). Scenarios marked `known_bug` must
//! fail with a mismatch; they are listed after the run. Two more tests check the card data:
//! `catalog` (against `tests/scenarios/catalog.yaml`) and `coverage` (every card source file
//! has a scenario file in `combat/` or `tavern/`).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use libtest_mimic::{Arguments, Failed, Trial};
use seaglass::cards::{self, spells, tokens};
use seaglass::scenario::{card_index, slug, CardKind, Failure, Header, Scenario, ScenarioFile};
use seaglass::{
    base_copies_for_tier, base_upgrade_cost, CardTemplate, DeityKind, DeityState, Unit,
};
use serde::Deserialize;

fn main() {
    let args = Arguments::from_args();
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.join("tests/scenarios");

    let mut trials = Vec::new();
    let mut known_bugs = BTreeMap::new();
    for path in yaml_files(&root) {
        let rel = path.strip_prefix(&root).expect("under the scenario root");
        let prefix: Vec<String> = rel
            .with_extension("")
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let prefix = prefix.join("::");
        if prefix == "catalog" {
            continue;
        }
        match ScenarioFile::load(&path) {
            Ok(file) => {
                for scenario in file.scenarios {
                    let name = format!("{prefix}::{}", scenario.name);
                    let trial = match scenario.known_bug.clone() {
                        None => Trial::test(name, move || scenario.run().map_err(Failed::from)),
                        Some(bug) => {
                            known_bugs.insert(name.clone(), bug.clone());
                            Trial::test(name, move || reproduces(&scenario, &bug))
                                .with_kind("known_bug")
                        }
                    };
                    trials.push(trial);
                }
            }
            Err(e) => {
                let message = format!("{}: {e}", rel.display());
                trials.push(Trial::test(format!("{prefix}::<file>"), move || {
                    Err(message.into())
                }));
            }
        }
    }
    let catalog = root.join("catalog.yaml");
    trials.push(Trial::test("catalog", move || check_catalog(&catalog)));
    let (src, scenarios) = (manifest.join("src/cards"), root.clone());
    trials.push(Trial::test("coverage", move || {
        check_coverage(&src, &scenarios)
    }));

    let ledger: Vec<String> = trials
        .iter()
        .filter(|t| t.kind() == "known_bug" && !args.is_filtered_out(t) && !args.is_ignored(t))
        .map(|t| format!("  {}: {}", t.name(), known_bugs[t.name()]))
        .collect();
    let conclusion = libtest_mimic::run(&args, trials);
    if !args.list && !ledger.is_empty() {
        println!("known bugs ({} scenarios expected to fail):", ledger.len());
        for line in &ledger {
            println!("{line}");
        }
        println!();
    }
    conclusion.exit();
}

/// A `known_bug` scenario must fail with a mismatch (the bug), not pass or be invalid.
fn reproduces(scenario: &Scenario, bug: &str) -> Result<(), Failed> {
    match scenario.run() {
        Err(Failure::Mismatch(_)) => Ok(()),
        Err(invalid @ Failure::Invalid(_)) => Err(invalid.into()),
        Ok(()) => Err(format!(
            "known bug no longer reproduces ({bug}): the scenario passes, remove `known_bug`"
        )
        .into()),
    }
}

/// Every `.yaml` file under `dir`, recursively, sorted.
fn yaml_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries =
            fs::read_dir(&dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "yaml") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// `catalog.yaml`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    minions: BTreeMap<u32, Vec<String>>,
    minion_ids: BTreeMap<u32, [u32; 2]>,
    tiers: BTreeMap<u32, TierRow>,
    spells: BTreeMap<u32, Vec<String>>,
    not_in_pool: Vec<String>,
    token_minions: Vec<String>,
    token_spells: Vec<String>,
    deities: Vec<String>,
}

/// One Tavern Tier's row of `catalog.yaml`'s `tiers` (Tier 7 is never a Tavern's tier: it
/// has no shop capacity or upgrade cost).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TierRow {
    copies: u32,
    shop_capacity: Option<usize>,
    upgrade_cost: Option<u32>,
}

/// How `got` differs from `want` (`None` if they are equal).
fn list_diff(what: &str, got: &[&str], want: &[String]) -> Option<String> {
    if got.iter().copied().eq(want.iter().map(String::as_str)) {
        return None;
    }
    let got_set: BTreeSet<&str> = got.iter().copied().collect();
    let want_set: BTreeSet<&str> = want.iter().map(String::as_str).collect();
    let missing: Vec<&str> = want_set.difference(&got_set).copied().collect();
    let extra: Vec<&str> = got_set.difference(&want_set).copied().collect();
    Some(if missing.is_empty() && extra.is_empty() {
        format!("{what}: same cards in a different order (got {got:?})")
    } else {
        format!(
            "{what}: missing {missing:?}, unexpected {extra:?} ({} cards, expected {})",
            got.len(),
            want.len()
        )
    })
}

fn template_names(templates: &[CardTemplate]) -> Vec<&str> {
    templates.iter().map(|t| t.name.as_str()).collect()
}

fn unit_names(units: &[Unit]) -> Vec<&str> {
    units.iter().map(|u| u.name.as_str()).collect()
}

/// How Tier `tier`'s pool copies, shop capacity and upgrade cost differ from `want`.
fn tier_row_diff(tier: u32, want: Option<&TierRow>) -> Option<String> {
    let Some(want) = want else {
        return Some(format!("catalog.yaml has no `tiers: {tier}:` row"));
    };
    // Tier 7 is never a Tavern's tier: no shop of its own, no upgrade from it.
    let tavern = (tier <= 6).then(|| (seaglass::shop_capacity(tier), base_upgrade_cost(tier)));
    let got = (
        base_copies_for_tier(tier),
        tavern.map(|t| t.0),
        tavern.map(|t| t.1),
    );
    let expected = (want.copies, want.shop_capacity, want.upgrade_cost);
    (got != expected).then(|| {
        format!(
            "tier {tier}: (copies, shop_capacity, upgrade_cost) is {got:?}, expected {expected:?}"
        )
    })
}

/// Check the catalogs, spell lists and card ids against `catalog.yaml`.
fn check_catalog(path: &Path) -> Result<(), Failed> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let want: Catalog =
        serde_yaml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut errors = Vec::new();
    let mut check = |diff: Option<String>| errors.extend(diff);

    let tiers: [fn() -> Vec<CardTemplate>; 7] = [
        seaglass::tier1_catalog,
        seaglass::tier2_catalog,
        seaglass::tier3_catalog,
        seaglass::tier4_catalog,
        seaglass::tier5_catalog,
        seaglass::tier6_catalog,
        seaglass::tier7_catalog,
    ];
    let solo: [fn() -> Vec<CardTemplate>; 7] = [
        cards::solo_tier_1_catalog,
        cards::solo_tier_2_catalog,
        cards::solo_tier_3_catalog,
        cards::solo_tier_4_catalog,
        cards::solo_tier_5_catalog,
        cards::solo_tier_6_catalog,
        cards::solo_tier_7_catalog,
    ];
    let spell_tiers: [fn() -> Vec<Unit>; 7] = [
        spells::tier1_spells,
        spells::tier2_spells,
        spells::tier3_spells,
        spells::tier4_spells,
        spells::tier5_spells,
        spells::tier6_spells,
        spells::tier7_spells,
    ];
    let mut all_minions: Vec<String> = Vec::new();
    let mut spells_so_far: Vec<String> = Vec::new();
    for (i, (catalog, tier_spells)) in tiers.iter().zip(spell_tiers).enumerate() {
        let tier = i as u32 + 1;
        let minions = want.minions.get(&tier).cloned().unwrap_or_default();
        let cards = catalog();
        check(list_diff(
            &format!("tier{tier}_catalog()"),
            &template_names(&cards),
            &minions,
        ));
        let by_name = seaglass::catalog_for(&format!("tier{tier}")).map_err(|e| e.to_string())?;
        check(list_diff(
            &format!("catalog_for(\"tier{tier}\")"),
            &template_names(&by_name),
            &minions,
        ));
        check(list_diff(
            &format!("solo_tier_{tier}_catalog()"),
            &template_names(&solo[i]()),
            &minions,
        ));
        let [lo, hi] = want.minion_ids.get(&tier).copied().unwrap_or_default();
        for card in &cards {
            if card.tavern_tier != tier {
                check(Some(format!(
                    "{} is in tier{tier}_catalog() but has tavern_tier {}",
                    card.name, card.tavern_tier
                )));
            }
            if !(lo..=hi).contains(&card.card_id) {
                check(Some(format!(
                    "{} has card id {}, outside tier {tier}'s range {lo}..={hi}",
                    card.name, card.card_id
                )));
            }
        }
        all_minions.extend(minions);
        check(tier_row_diff(tier, want.tiers.get(&tier)));

        let tier_spell_names = want.spells.get(&tier).cloned().unwrap_or_default();
        check(list_diff(
            &format!("spells::tier{tier}_spells()"),
            &unit_names(&tier_spells()),
            &tier_spell_names,
        ));
        spells_so_far.extend(tier_spell_names);
        let up_to = spells::spells_up_to_tier(tier);
        check(list_diff(
            &format!("spells::spells_up_to_tier({tier})"),
            &unit_names(&up_to),
            &spells_so_far,
        ));
    }
    check(list_diff(
        "full_catalog()",
        &template_names(&seaglass::full_catalog()),
        &all_minions,
    ));

    let tavern_spells = spells::spells_up_to_tier(7);
    let not_in_pool: Vec<Unit> = tavern_spells
        .iter()
        .filter(|s| !spells::is_pool_tavern_spell(s.card_id))
        .cloned()
        .collect();
    check(list_diff(
        "Tavern spells not in the pool",
        &unit_names(&not_in_pool),
        &want.not_in_pool,
    ));
    let all_spells = spells::all_spells();
    check(list_diff(
        "token spells",
        &unit_names(&all_spells[tavern_spells.len()..]),
        &want.token_spells,
    ));
    let token_minions = tokens::all_token_minions();
    check(list_diff(
        "token minions",
        &unit_names(&token_minions),
        &want.token_minions,
    ));
    let deities = [DeityKind::CThun, DeityKind::YShaarj]
        .map(|kind| cards::instantiate_deity(&DeityState::new(kind)));
    check(list_diff("deities", &unit_names(&deities), &want.deities));

    // Card ids and names are unique across every card.
    let mut ids: BTreeMap<u32, String> = BTreeMap::new();
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    let everything = seaglass::full_catalog()
        .into_iter()
        .map(|t| (t.card_id, t.name))
        .chain(
            all_spells
                .iter()
                .chain(&token_minions)
                .chain(&deities)
                .map(|u| (u.card_id, u.name.clone())),
        );
    for (id, name) in everything {
        if let Some(other) = ids.insert(id, name.clone()) {
            check(Some(format!(
                "card id {id} is used by both {other} and {name}"
            )));
        }
        if let Some(other) = names.insert(name.to_ascii_lowercase(), name.clone()) {
            check(Some(format!(
                "card name {name:?} is used twice (also {other:?})"
            )));
        }
    }
    if card_index().cards().len() != ids.len() {
        check(Some(format!(
            "the scenario card index has {} cards, expected {}",
            card_index().cards().len(),
            ids.len()
        )));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n").into())
    }
}

/// The stems of the files in `dir` with extension `ext`.
fn stems(dir: &Path, ext: &str) -> BTreeSet<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return BTreeSet::new();
    };
    entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == ext))
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect()
}

/// Every card source file has a scenario file in `combat/` or `tavern/`, and every scenario
/// file belongs to a card or a topic.
fn check_coverage(src: &Path, root: &Path) -> Result<(), Failed> {
    let mut errors = Vec::new();
    let mut minions = stems(&src.join("minions"), "rs");
    minions.remove("mod");
    let mut spells = stems(&src.join("spells"), "rs");
    spells.remove("mod");

    let combat_files = stems(&root.join("combat"), "yaml");
    let tavern_files = stems(&root.join("tavern"), "yaml");

    for stem in &minions {
        match (combat_files.contains(stem), tavern_files.contains(stem)) {
            (false, false) => errors.push(format!(
                "missing scenario file for src/cards/minions/{stem}.rs (expected in combat/ or tavern/)"
            )),
            (true, true) => errors.push(format!(
                "{stem}.yaml appears in both combat/ and tavern/"
            )),
            _ => {}
        }
    }
    for stem in &spells {
        if combat_files.contains(stem) {
            errors.push(format!(
                "combat/{stem}.yaml: spell scenarios belong in tavern/{stem}.yaml"
            ));
        } else if !tavern_files.contains(stem) {
            errors.push(format!(
                "tavern/{stem}.yaml is missing (for src/cards/spells/{stem}.rs)"
            ));
        }
    }

    for (dir, files) in [("combat", &combat_files), ("tavern", &tavern_files)] {
        for stem in files {
            let file = format!("{dir}/{stem}.yaml");
            let expected_kind = if minions.contains(stem) {
                Some(CardKind::Minion)
            } else if dir == "tavern" && spells.contains(stem) {
                Some(CardKind::Spell)
            } else {
                None
            };
            match ScenarioFile::load(&root.join(&file)).map(|f| f.header) {
                Ok(Header::Card(name)) => match expected_kind {
                    Some(kind) => errors.extend(misplaced(&file, stem, &name, kind)),
                    None => errors.push(format!(
                        "{file} has no src/cards/{{minions,spells}}/{stem}.rs"
                    )),
                },
                Ok(Header::Topic(_)) => {
                    if expected_kind.is_some() {
                        errors.push(format!("{file} must start with `card:`"));
                    }
                }
                Err(e) => errors.push(format!("{file}: {e}")),
            }
        }
    }

    let known = ["combat", "tavern", "catalog.yaml"];
    for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
        let name = entry
            .map_err(|e| e.to_string())?
            .file_name()
            .to_string_lossy()
            .into_owned();
        if !known.contains(&name.as_str()) {
            errors.push(format!(
                "unexpected {name} in tests/scenarios (expected {})",
                known.join(", ")
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n").into())
    }
}

/// Why the card scenario file `file` (named `stem`, about the card `name`) is misplaced, if
/// it is: its name must be the card's slug, and its directory must match the card's kind.
fn misplaced(file: &str, stem: &str, name: &str, kind: CardKind) -> Option<String> {
    if slug(name) != stem {
        return Some(format!(
            "{file} is about {name}, whose file would be {}.yaml",
            slug(name)
        ));
    }
    match card_index().resolve(name).map(|c| c.kind) {
        Ok(k) if k != kind => Some(format!("{file}: {name} is a {k:?}, expected a {kind:?}")),
        Ok(_) => None,
        Err(e) => Some(format!("{file}: {e}")),
    }
}
