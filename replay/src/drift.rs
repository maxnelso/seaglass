//! Patch drift: where the log's game data differs from the engine's (the log can come from an
//! older or newer patch than the one the engine targets).
//!
//! Every minion the log creates (`FULL_ENTITY`, or `SHOW_ENTITY` for one created hidden) shows its
//! card's numbers: `TECH_LEVEL`, `ATK` / `HEALTH` at creation, its keywords and its types
//! (`CARDRACE` names the first type; a typed pool minion's `BACON_SUBSET_*` tags name all of its
//! types, while a neutral one's name the lobby types it needs). [`observe_cards`]
//! collects them per card ID (shop copies, hands, boards, tokens summoned in combat), and
//! [`card_drift`] compares them with the engine's templates, tokens and Deities:
//!
//! - Tier and types never change in a game: the most common value must be the engine's.
//! - Stats and keywords grow with buffs (board copies, copies of buffed minions): they drift only
//!   if *no* creation shows the engine's values (then the most common one is reported).
//! - Golden copies are compared with double stats.
//!
//! [`rule_drift`] compares game numbers: the Deity's awakening count (`BG_OldGod`'s
//! `QUEST_PROGRESS_TOTAL`); an awakened Deity's Tier is a card drift of `C'Thun` / `Y'Shaarj`.
//! [`report`] puts it together for a parsed log, with its build; `replay_cli drift` prints it and
//! `replay_cli validate` uses it to tell patch drift from engine bugs. Nothing is specific to one
//! patch: run it on a log from any build.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use seaglass::cards::{self, tokens};
use seaglass::model::{CardId, DeityKind, DeityState, Tribe, Unit, SINGLE_TRIBES};

use crate::cards::{engine_card, CardKind, ENGINE_PATCH};
use crate::GameReplay;

/// What the log shows of one card across its creations.
#[derive(Clone, Debug, Default, Serialize)]
pub struct CardObservation {
    pub name: String,
    pub golden: bool,
    /// Creations seen.
    pub seen: u32,
    pub stats: BTreeMap<(i32, i32), u32>,
    pub tiers: BTreeMap<u32, u32>,
    pub keywords: BTreeMap<Vec<String>, u32>,
    /// `CARDRACE` (its first type; `""`: none), as an engine type name.
    pub races: BTreeMap<String, u32>,
    /// All types of a typed pool minion: its `BACON_SUBSET_*` tags (the lobby types it needs,
    /// which for a typed minion are its types; neutral minions and Goldens have no types here).
    pub pool_types: BTreeMap<Vec<String>, u32>,
    /// `QUEST_PROGRESS_TOTAL` (the Deity's `BG_OldGod`).
    pub quest_totals: BTreeMap<i32, u32>,
}

const KEYWORD_TAGS: [(&str, &str); 9] = [
    ("TAUNT", "taunt"),
    ("DIVINE_SHIELD", "divine_shield"),
    ("WINDFURY", "windfury"),
    ("REBORN", "reborn"),
    ("VENOMOUS", "venomous"),
    ("POISONOUS", "venomous"),
    ("STEALTH", "stealth"),
    ("MODULAR", "magnetic"),
    ("MAGNETIC", "magnetic"),
];

/// `CARDRACE` values and `BACON_SUBSET_*` tags, as engine type names.
const RACES: [(&str, &str); 12] = [
    ("ABERRATION", "aberration"),
    ("BEAST", "beast"),
    ("DEMON", "demon"),
    ("DRAGON", "dragon"),
    ("ELEMENTAL", "elemental"),
    ("MECHANICAL", "mech"),
    ("MURLOC", "murloc"),
    ("PIRATE", "pirate"),
    ("QUILBOAR", "quilboar"),
    ("UNDEAD", "undead"),
    ("NAGA", "naga"),
    ("ALL", "all"),
];

const SUBSETS: [(&str, &str); 11] = [
    ("BACON_SUBSET_ABERRATION", "aberration"),
    ("BACON_SUBSET_BEAST", "beast"),
    ("BACON_SUBSET_DEMON", "demon"),
    ("BACON_SUBSET_DRAGON", "dragon"),
    ("BACON_SUBSET_ELEMENTALS", "elemental"),
    ("BACON_SUBSET_MECH", "mech"),
    ("BACON_SUBSET_MURLOC", "murloc"),
    ("BACON_SUBSET_PIRATE", "pirate"),
    ("BACON_SUBSET_QUILLBOAR", "quilboar"),
    ("BACON_SUBSET_UNDEAD", "undead"),
    ("BACON_SUBSET_NAGA", "naga"),
];

fn record(
    out: &mut BTreeMap<String, CardObservation>,
    card_id: &str,
    tags: &BTreeMap<String, String>,
    card_names: &std::collections::HashMap<String, String>,
) {
    let get = |k: &str| tags.get(k).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
    if card_id == "BG_OldGod" {
        let o = out.entry(card_id.to_string()).or_default();
        o.name = "Secret Deity".into();
        o.seen += 1;
        *o.quest_totals
            .entry(get("QUEST_PROGRESS_TOTAL"))
            .or_default() += 1;
        return;
    }
    if card_id.is_empty() || tags.get("CARDTYPE").map(String::as_str) != Some("MINION") {
        return;
    }
    let o = out.entry(card_id.to_string()).or_default();
    if o.name.is_empty() {
        o.name = card_names.get(card_id).cloned().unwrap_or_default();
    }
    o.golden |= get("PREMIUM") > 0;
    o.seen += 1;
    *o.stats.entry((get("ATK"), get("HEALTH"))).or_default() += 1;
    *o.tiers.entry(get("TECH_LEVEL").max(0) as u32).or_default() += 1;
    let mut kws: Vec<String> = KEYWORD_TAGS
        .iter()
        .filter(|(tag, _)| get(tag) > 0)
        .map(|(_, kw)| kw.to_string())
        .collect();
    kws.sort();
    kws.dedup();
    *o.keywords.entry(kws).or_default() += 1;
    let race = tags
        .get("CARDRACE")
        .and_then(|r| RACES.iter().find(|(k, _)| k == r))
        .map_or("", |(_, t)| t);
    *o.races.entry(race.to_string()).or_default() += 1;
    let subsets: Vec<String> = SUBSETS
        .iter()
        .filter(|(tag, _)| get(tag) > 0)
        .map(|(_, t)| t.to_string())
        .collect();
    if get("IS_BACON_POOL_MINION") > 0 && subsets.iter().any(|t| t == race) {
        *o.pool_types.entry(subsets).or_default() += 1;
    }
}

/// Collects what one game's log shows of every minion card (and the Deity's `BG_OldGod`), from
/// its `GameState.DebugPrintPower()` lines (prefix removed).
pub fn observe_cards(
    lines: &[&str],
    card_names: &std::collections::HashMap<String, String>,
) -> BTreeMap<String, CardObservation> {
    let mut out = BTreeMap::new();
    let mut cur: Option<(String, usize)> = None;
    let mut tags: BTreeMap<String, String> = BTreeMap::new();
    for &raw in lines {
        let indent = raw.len() - raw.trim_start().len();
        let s = raw.trim();
        if let Some((cid, init_indent)) = &cur {
            if let Some(rest) = s.strip_prefix("tag=") {
                if indent > *init_indent {
                    if let Some((k, v)) = rest.split_once(" value=") {
                        tags.insert(k.trim().to_string(), v.trim().to_string());
                    }
                    continue;
                }
            }
            record(&mut out, cid, &tags, card_names);
            cur = None;
        }
        let cid = s
            .strip_prefix("FULL_ENTITY - Creating ID=")
            .and_then(|r| r.split_once(" CardID=").map(|(_, c)| c))
            .or_else(|| {
                s.strip_prefix("SHOW_ENTITY - Updating Entity=")
                    .and_then(|r| r.rsplit_once(" CardID=").map(|(_, c)| c))
            });
        if let Some(cid) = cid {
            cur = Some((cid.trim().to_string(), indent));
            tags.clear();
        }
    }
    if let Some((cid, _)) = cur {
        record(&mut out, &cid, &tags, card_names);
    }
    out
}

/// Merges observations (several games).
pub fn merge(
    into: &mut BTreeMap<String, CardObservation>,
    from: &BTreeMap<String, CardObservation>,
) {
    for (cid, o) in from {
        let e = into.entry(cid.clone()).or_default();
        if e.name.is_empty() {
            e.name = o.name.clone();
        }
        e.golden |= o.golden;
        e.seen += o.seen;
        for (k, n) in &o.stats {
            *e.stats.entry(*k).or_default() += n;
        }
        for (k, n) in &o.tiers {
            *e.tiers.entry(*k).or_default() += n;
        }
        for (k, n) in &o.keywords {
            *e.keywords.entry(k.clone()).or_default() += n;
        }
        for (k, n) in &o.races {
            *e.races.entry(k.clone()).or_default() += n;
        }
        for (k, n) in &o.pool_types {
            *e.pool_types.entry(k.clone()).or_default() += n;
        }
        for (k, n) in &o.quest_totals {
            *e.quest_totals.entry(*k).or_default() += n;
        }
    }
}

/// One card whose log data differs from the engine's.
#[derive(Clone, Debug, Serialize)]
pub struct CardDrift {
    pub card_id: String,
    pub name: String,
    pub golden: bool,
    pub seen: u32,
    /// `"stats 3/4 (14 of 15) vs engine 2/2"`, `"tier 3 vs engine 1"`, ...
    pub diffs: Vec<String>,
}

fn mode<K: Clone + Ord>(m: &BTreeMap<K, u32>) -> Option<(K, u32)> {
    m.iter()
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(k, n)| (k.clone(), *n))
}

fn engine_unit(card_id: CardId, kind: CardKind) -> Option<Unit> {
    match kind {
        CardKind::Minion => cards::template(card_id).map(|t| t.instantiate()),
        CardKind::Token => tokens::make_plain_token(
            &Unit::new("", 0, 0).with_card_id(card_id),
            &Default::default(),
        ),
        CardKind::Deity => {
            let kind = if card_id == cards::deities::CARD_YSHAARJ {
                DeityKind::YShaarj
            } else {
                DeityKind::CThun
            };
            Some(cards::instantiate_deity(&DeityState::new(kind)))
        }
        CardKind::Spell => None,
    }
}

/// The engine's Tier for a card (its template, token or Deity), if the engine has it.
pub fn engine_tier(hs_id: &str, name: &str) -> Option<u32> {
    let card = engine_card(hs_id, name)?;
    engine_unit(card.card_id, card.kind).map(|u| u.tavern_tier)
}

fn tribe_names(tribe: Tribe) -> Vec<String> {
    let mut v: Vec<String> = SINGLE_TRIBES
        .iter()
        .filter(|&&t| tribe.has_single(t))
        .map(|t| t.as_str().to_string())
        .collect();
    v.sort();
    v
}

/// The cards whose most common log data differs from the engine's.
pub fn card_drift(obs: &BTreeMap<String, CardObservation>) -> Vec<CardDrift> {
    let mut out = Vec::new();
    for (cid, o) in obs {
        if cid == "BG_OldGod" || o.seen == 0 {
            continue;
        }
        let Some(card) = engine_card(cid, &o.name) else {
            continue;
        };
        let Some(u) = engine_unit(card.card_id, card.kind) else {
            continue;
        };
        let mult = if o.golden { 2 } else { 1 };
        let mut diffs = Vec::new();
        let (ea, eh) = (u.attack * mult, u.health * mult);
        if card.kind != CardKind::Deity && !o.stats.contains_key(&(ea, eh)) {
            if let Some(((a, h), n)) = mode(&o.stats) {
                diffs.push(format!(
                    "stats {a}/{h} ({n} of {} creations) vs engine {ea}/{eh}",
                    o.seen
                ));
            }
        }
        if let Some((tier, n)) = mode(&o.tiers) {
            if tier != u.tavern_tier && tier > 0 {
                diffs.push(format!(
                    "Tier {tier} ({n} of {} creations) vs engine {}",
                    o.seen, u.tavern_tier
                ));
            }
        }
        {
            let mut ekws: Vec<String> = [
                (u.taunt, "taunt"),
                (u.divine_shield, "divine_shield"),
                (u.windfury, "windfury"),
                (u.reborn, "reborn"),
                (u.venomous, "venomous"),
                (u.stealth, "stealth"),
                (u.magnetic, "magnetic"),
            ]
            .iter()
            .filter(|(on, _)| *on)
            .map(|(_, kw)| kw.to_string())
            .collect();
            ekws.sort();
            if !o.keywords.contains_key(&ekws) {
                if let Some((kws, n)) = mode(&o.keywords) {
                    diffs.push(format!(
                        "keywords [{}] ({n} of {} creations) vs engine [{}]",
                        kws.join(", "),
                        o.seen,
                        ekws.join(", ")
                    ));
                }
            }
        }
        let engine_types = tribe_names(u.tribe);
        if let Some((race, n)) = mode(&o.races) {
            let fits = match race.as_str() {
                "" => engine_types.is_empty(),
                "all" => u.tribe == Tribe::All,
                r => engine_types.iter().any(|t| t == r),
            };
            if !fits {
                diffs.push(format!(
                    "type {} ({n} of {} creations) vs engine [{}]",
                    if race.is_empty() { "none" } else { &race },
                    o.seen,
                    engine_types.join(", ")
                ));
            }
        }
        if let Some((types, n)) = mode(&o.pool_types) {
            if u.tribe != Tribe::All && types != engine_types {
                diffs.push(format!(
                    "types [{}] ({n} of {} pool creations) vs engine [{}]",
                    types.join(", "),
                    o.pool_types.values().sum::<u32>(),
                    engine_types.join(", ")
                ));
            }
        }
        if !diffs.is_empty() {
            out.push(CardDrift {
                card_id: cid.clone(),
                name: if o.name.is_empty() {
                    card.name.clone()
                } else {
                    o.name.clone()
                },
                golden: o.golden,
                seen: o.seen,
                diffs,
            });
        }
    }
    out
}

/// Game numbers that differ from the engine's (the Deity's awakening count).
pub fn rule_drift(obs: &BTreeMap<String, CardObservation>) -> Vec<String> {
    let mut out = Vec::new();
    if let Some((total, n)) = obs.get("BG_OldGod").and_then(|o| mode(&o.quest_totals)) {
        let engine = cards::DEITY_SACRIFICE_REQUIREMENT as i32;
        if total > 0 && total != engine {
            out.push(format!(
                "the Deity awakens after {total} friendly Aberration deaths (BG_OldGod QUEST_PROGRESS_TOTAL, {n} creations) vs engine {engine}"
            ));
        }
    }
    out
}

/// The names (lowercase) of the Deities once awakened.
pub const DEITY_NAMES: [&str; 2] = ["c'thun", "y'shaarj"];

/// Where a log's game data differs from the engine's.
#[derive(Clone, Debug, Default, Serialize)]
pub struct DriftReport {
    /// Each game's client build: `(game index, BuildNumber)`.
    pub builds: Vec<(usize, Option<u32>)>,
    /// The patch the engine targets.
    pub engine_patch: String,
    /// Minion cards seen in the log.
    pub cards_seen: usize,
    pub cards: Vec<CardDrift>,
    pub rules: Vec<String>,
}

impl DriftReport {
    /// The drift of the card named `name` (any case; a Deity when the Deity rules drifted).
    pub fn find(&self, name: &str) -> Option<String> {
        let key = name.trim().trim_end_matches(" ★").to_ascii_lowercase();
        let key = key.strip_prefix("golden ").unwrap_or(&key);
        if let Some(d) = self.cards.iter().find(|d| d.name.eq_ignore_ascii_case(key)) {
            return Some(format!("{}: {}", d.name, d.diffs.join(", ")));
        }
        if DEITY_NAMES.contains(&key) {
            if let Some(r) = self.rules.first() {
                return Some(format!("{name}: {r}"));
            }
        }
        None
    }

    /// Lowercase names of the drifted cards (and of the Deities, when the Deity rules drifted).
    pub fn names(&self) -> BTreeSet<String> {
        let mut names: BTreeSet<String> = self
            .cards
            .iter()
            .map(|d| d.name.to_ascii_lowercase())
            .collect();
        if !self.rules.is_empty() {
            names.extend(DEITY_NAMES.iter().map(|n| n.to_string()));
        }
        names
    }
}

/// The patch drift of a parsed log (all its games).
pub fn report(games: &[GameReplay]) -> DriftReport {
    let mut obs = BTreeMap::new();
    for g in games {
        merge(&mut obs, &g.cards_observed);
    }
    DriftReport {
        builds: games
            .iter()
            .map(|g| (g.game_index, g.build_number))
            .collect(),
        engine_patch: ENGINE_PATCH.to_string(),
        cards_seen: obs.keys().filter(|k| *k != "BG_OldGod").count(),
        cards: card_drift(&obs),
        rules: rule_drift(&obs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn creation(id: u32, card_id: &str, tags: &[(&str, &str)]) -> Vec<String> {
        let mut lines = vec![format!("FULL_ENTITY - Creating ID={id} CardID={card_id}")];
        lines.extend(tags.iter().map(|(k, v)| format!("    tag={k} value={v}")));
        lines
    }

    fn observe(creations: Vec<Vec<String>>) -> BTreeMap<String, CardObservation> {
        let lines: Vec<String> = creations.into_iter().flatten().collect();
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        observe_cards(&refs, &Default::default())
    }

    fn minion(
        atk: &str,
        hp: &str,
        tier: &str,
        extra: &[(&'static str, &'static str)],
    ) -> Vec<(&'static str, String)> {
        let mut v = vec![
            ("CARDTYPE", "MINION".to_string()),
            ("ATK", atk.to_string()),
            ("HEALTH", hp.to_string()),
            ("TECH_LEVEL", tier.to_string()),
        ];
        v.extend(extra.iter().map(|(k, v)| (*k, v.to_string())));
        v
    }

    fn tags<'a>(v: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
        v.iter().map(|(k, s)| (*k, s.as_str())).collect()
    }

    #[test]
    fn matching_cards_do_not_drift() {
        // Vicious Mindslasher as the engine has it, plus a buffed board copy.
        let t = cards::template(crate::cards::engine_card("BG36_108", "").unwrap().card_id)
            .unwrap()
            .instantiate();
        let (a, h, tier) = (
            t.attack.to_string(),
            t.health.to_string(),
            t.tavern_tier.to_string(),
        );
        let plain = minion(&a, &h, &tier, &[("CARDRACE", "ABERRATION")]);
        let buffed = minion(
            "30",
            "40",
            &tier,
            &[("CARDRACE", "ABERRATION"), ("TAUNT", "1")],
        );
        let obs = observe(vec![
            creation(10, "BG36_108", &tags(&buffed)),
            creation(11, "BG36_108", &tags(&buffed)),
            creation(12, "BG36_108", &tags(&plain)),
        ]);
        assert_eq!(obs["BG36_108"].seen, 3);
        assert!(card_drift(&obs).is_empty(), "{:?}", card_drift(&obs));
    }

    #[test]
    fn changed_cards_and_deity_rules_drift() {
        let t = cards::template(crate::cards::engine_card("BG36_108", "").unwrap().card_id)
            .unwrap()
            .instantiate();
        let other_tier = (t.tavern_tier % 6 + 1).to_string();
        let changed = minion(
            &(t.attack + 2).to_string(),
            &(t.health + 1).to_string(),
            &other_tier,
            &[("CARDRACE", "ABERRATION")],
        );
        let old_god = [("CARDTYPE", "MINION"), ("QUEST_PROGRESS_TOTAL", "3")];
        let obs = observe(vec![
            creation(10, "BG36_108", &tags(&changed)),
            creation(11, "BG_OldGod", &old_god),
        ]);
        let drift = card_drift(&obs);
        assert_eq!(drift.len(), 1);
        assert_eq!(drift[0].card_id, "BG36_108");
        assert_eq!(drift[0].diffs.len(), 2, "{:?}", drift[0].diffs);
        assert!(drift[0].diffs[0].starts_with("stats "));
        assert!(drift[0].diffs[1].starts_with(&format!("Tier {other_tier} ")));
        let rules = rule_drift(&obs);
        assert_eq!(
            rules.len(),
            (cards::DEITY_SACRIFICE_REQUIREMENT != 3) as usize
        );
    }
}
