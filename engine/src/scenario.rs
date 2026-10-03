//! Declarative YAML combat scenario parser and runner (`docs/scenarios.md` Part 1).

use serde::Deserialize;

use crate::cards::tier1;
use crate::model::{BattleOutcome, DeityKind, GameState, Keyword, Tribe, Unit};
use crate::sim::{simulate, simulate_batch};

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    pub tavern_tier: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultSpec {
    TeamAWin,
    TeamBWin,
    Draw,
}

impl ResultSpec {
    fn matches(self, outcome: BattleOutcome) -> bool {
        matches!(
            (self, outcome),
            (ResultSpec::TeamAWin, BattleOutcome::AWin)
                | (ResultSpec::TeamBWin, BattleOutcome::BWin)
                | (ResultSpec::Draw, BattleOutcome::Draw)
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    pub result: ResultSpec,
    pub survivors_a: Option<Vec<String>>,
    pub survivors_b: Option<Vec<String>>,
    pub hero_damage: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub base_seed: u64,
    pub n: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Range {
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectStats {
    pub a_win_rate: Option<Range>,
    pub b_win_rate: Option<Range>,
    pub draw_rate: Option<Range>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub name: String,
    #[serde(default)]
    pub seed: u64,
    #[serde(default = "default_hero_tier")]
    pub hero_tier_a: u32,
    #[serde(default = "default_hero_tier")]
    pub hero_tier_b: u32,
    pub deity_a: Option<DeityKind>,
    pub deity_b: Option<DeityKind>,
    pub deity_stats_a: Option<[i32; 2]>,
    pub deity_stats_b: Option<[i32; 2]>,
    #[serde(default)]
    pub defaults: Defaults,
    pub team_a: Vec<String>,
    pub team_b: Vec<String>,
    pub expect: Option<Expect>,
    pub batch: Option<Batch>,
    pub expect_stats: Option<ExpectStats>,
}

fn default_hero_tier() -> u32 {
    1
}

/// Parse a compact unit specification string (`"<attack>/<health> [tokens...]"`).
pub fn parse_unit(spec: &str, defaults: &Defaults) -> Result<Unit, String> {
    let mut tokens = spec.split_whitespace();
    let stats_tok = tokens
        .next()
        .ok_or_else(|| "empty unit spec".to_string())?;
    let (atk_str, hp_str) = stats_tok
        .split_once('/')
        .ok_or_else(|| format!("unit spec {spec:?} must start with '<attack>/<health>'"))?;

    let attack: i32 = atk_str
        .parse()
        .map_err(|_| format!("invalid attack {atk_str:?} in {spec:?}"))?;
    let health: i32 = hp_str
        .parse()
        .map_err(|_| format!("invalid health {hp_str:?} in {spec:?}"))?;

    let mut unit = Unit::new(spec, attack, health);
    if let Some(tier) = defaults.tavern_tier {
        unit.tavern_tier = tier;
    }

    for tok in tokens {
        match tok {
            // Keywords
            "taunt" => unit.apply_keyword(Keyword::Taunt, true),
            "divine_shield" => unit.apply_keyword(Keyword::DivineShield, true),
            "windfury" => unit.apply_keyword(Keyword::Windfury, true),
            "reborn" => unit.apply_keyword(Keyword::Reborn, true),
            "venomous" => unit.apply_keyword(Keyword::Venomous, true),
            "stealth" => unit.apply_keyword(Keyword::Stealth, true),
            "magnetic" => unit.apply_keyword(Keyword::Magnetic, true),
            "golden" => unit.is_golden = true,
            // Tribes
            "aberration" => unit.tribe = Tribe::Aberration,
            "beast" => unit.tribe = Tribe::Beast,
            "demon" => unit.tribe = Tribe::Demon,
            "dragon" => unit.tribe = Tribe::Dragon,
            "elemental" => unit.tribe = Tribe::Elemental,
            "mech" => unit.tribe = Tribe::Mech,
            "murloc" => unit.tribe = Tribe::Murloc,
            "pirate" => unit.tribe = Tribe::Pirate,
            "quilboar" => unit.tribe = Tribe::Quilboar,
            "undead" => unit.tribe = Tribe::Undead,
            "all" => unit.tribe = Tribe::All,
            // Card-linked triggers and template lookup for scenario authoring
            "deathrattle_beetle" => unit.card_id = tier1::buzzing_vermin::ID,
            "deathrattle_microbot" => unit.card_id = tier1::cord_puller::ID,
            "deathrattle_skeletons" => unit.card_id = tier1::harmless_bonehead::ID,
            "rally_bat" => unit.card_id = tier1::flittering_bat::ID,
            "rally_glim" => unit.card_id = tier1::glim_guardian::ID,
            "rally_camper" => unit.card_id = tier1::tusked_camper::ID,
            "scarlet_survivor" => unit.card_id = tier1::scarlet_survivor::ID,
            other if other.starts_with("card:") => {
                let slug = &other["card:".len()..];
                let tpl = crate::cards::full_catalog()
                    .into_iter()
                    .find(|t| {
                        let t_slug = t
                            .name
                            .to_ascii_lowercase()
                            .replace([' ', '-', '\''], "_");
                        t_slug == slug
                    })
                    .ok_or_else(|| format!("unknown card slug {slug:?} in {spec:?}"))?;
                unit.card_id = tpl.card_id;
                unit.name = tpl.name;
                unit.tavern_tier = tpl.tavern_tier;
                unit.tribe = tpl.tribe;
                unit.taunt |= tpl.taunt;
                unit.divine_shield |= tpl.divine_shield;
                unit.inherent_divine_shield |= tpl.divine_shield;
                unit.windfury |= tpl.windfury;
                unit.reborn |= tpl.reborn;
                unit.venomous |= tpl.venomous;
                unit.stealth |= tpl.stealth;
                unit.magnetic |= tpl.magnetic;
                if tpl.intrinsic_golden {
                    unit.is_golden = true;
                    unit.intrinsic_golden = true;
                }
            }
            other if other.starts_with("tier:") => {
                let tier: u32 = other["tier:".len()..]
                    .parse()
                    .map_err(|_| format!("invalid tier token {other:?} in {spec:?}"))?;
                unit.tavern_tier = tier;
            }
            other if other.starts_with('t') && other.len() > 1 => {
                let tier: u32 = other[1..]
                    .parse()
                    .map_err(|_| format!("unknown token {other:?} in {spec:?}"))?;
                unit.tavern_tier = tier;
            }
            other => return Err(format!("unknown token {other:?} in {spec:?}")),
        }
    }

    crate::cards::check_stat_thresholds(&mut unit);
    Ok(unit)
}

fn survivors_match(actual: &[Unit], expected_specs: &[String], defaults: &Defaults) -> Result<(), String> {
    if actual.len() != expected_specs.len() {
        return Err(format!(
            "expected {} survivor(s), got {}: actual={:?}",
            expected_specs.len(),
            actual.len(),
            actual
                .iter()
                .map(|u| format!("{}/{}", u.attack, u.health))
                .collect::<Vec<_>>()
        ));
    }

    for (i, (act, spec)) in actual.iter().zip(expected_specs.iter()).enumerate() {
        let exp = parse_unit(spec, defaults)?;
        if act.attack != exp.attack
            || act.health != exp.health
            || act.taunt != exp.taunt
            || act.divine_shield != exp.divine_shield
            || act.windfury != exp.windfury
            || act.reborn != exp.reborn
        {
            return Err(format!(
                "survivor[{i}] mismatch: expected {spec:?}, got {}/{} (taunt={}, shield={}, windfury={}, reborn={})",
                act.attack, act.health, act.taunt, act.divine_shield, act.windfury, act.reborn
            ));
        }
    }
    Ok(())
}

/// Build `(board_a, board_b, game_state)` from a parsed [`Scenario`].
pub fn teams_and_state(scenario: &Scenario) -> Result<(Vec<Unit>, Vec<Unit>, GameState), String> {
    let board_a: Vec<Unit> = scenario
        .team_a
        .iter()
        .map(|s| parse_unit(s, &scenario.defaults))
        .collect::<Result<_, _>>()?;
    let board_b: Vec<Unit> = scenario
        .team_b
        .iter()
        .map(|s| parse_unit(s, &scenario.defaults))
        .collect::<Result<_, _>>()?;

    let mut state = GameState {
        hero_tier_a: scenario.hero_tier_a,
        hero_tier_b: scenario.hero_tier_b,
        ..GameState::default()
    };
    if let Some(kind) = scenario.deity_a {
        state.auras_a.deity.kind = kind;
    }
    if let Some(kind) = scenario.deity_b {
        state.auras_b.deity.kind = kind;
    }
    if let Some([atk, hp]) = scenario.deity_stats_a {
        state.auras_a.deity.attack = atk;
        state.auras_a.deity.health = hp;
    }
    if let Some([atk, hp]) = scenario.deity_stats_b {
        state.auras_b.deity.attack = atk;
        state.auras_b.deity.health = hp;
    }

    Ok((board_a, board_b, state))
}

/// Run a combat [`Scenario`] and verify its exact or statistical expectations.
pub fn run_scenario(scenario: &Scenario) -> Result<(), String> {
    let (board_a, board_b, state) = teams_and_state(scenario)?;

    match (&scenario.expect, &scenario.batch, &scenario.expect_stats) {
        (Some(exp), None, None) => {
            let res = simulate(&board_a, &board_b, &state, scenario.seed);
            if !exp.result.matches(res.outcome) {
                return Err(format!(
                    "[{}] expected {:?}, got {:?}",
                    scenario.name, exp.result, res.outcome
                ));
            }
            if let Some(ref want_a) = exp.survivors_a {
                survivors_match(&res.survivors_a, want_a, &scenario.defaults)
                    .map_err(|e| format!("[{}] survivors_a: {e}", scenario.name))?;
            }
            if let Some(ref want_b) = exp.survivors_b {
                survivors_match(&res.survivors_b, want_b, &scenario.defaults)
                    .map_err(|e| format!("[{}] survivors_b: {e}", scenario.name))?;
            }
            if let Some(dmg) = exp.hero_damage {
                if res.hero_damage != dmg {
                    return Err(format!(
                        "[{}] expected hero_damage {dmg}, got {}",
                        scenario.name, res.hero_damage
                    ));
                }
            }
            Ok(())
        }
        (None, Some(batch), Some(stats)) => {
            let dist = simulate_batch(&board_a, &board_b, &state, batch.base_seed, batch.n);
            let check = |label: &str, actual: f64, range: &Option<Range>| -> Result<(), String> {
                if let Some(r) = range {
                    if actual < r.min || actual > r.max {
                        return Err(format!(
                            "[{}] {label} {actual:.4} outside [{:.4}, {:.4}]",
                            scenario.name, r.min, r.max
                        ));
                    }
                }
                Ok(())
            };
            check("a_win_rate", dist.a_win_rate, &stats.a_win_rate)?;
            check("b_win_rate", dist.b_win_rate, &stats.b_win_rate)?;
            check("draw_rate", dist.draw_rate, &stats.draw_rate)?;
            Ok(())
        }
        _ => Err(format!(
            "[{}] scenario must set either `expect` (exact) or `batch` + `expect_stats` (statistical)",
            scenario.name
        )),
    }
}
