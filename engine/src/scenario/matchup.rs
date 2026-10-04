//! Board-vs-board matchup files (`examples/matchups/*.yaml`, run by `combat_cli`) and the compact
//! unit spec strings they use (`"3/2 divine_shield card:zoatroid"`, also exposed to Python as
//! `parse_unit`). The test suite's scenario format is the parent [`scenario`](super) module.

use serde::{Deserialize, Serialize};

use crate::model::{DeityKind, GameState, Keyword, Tribe, Unit};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tavern_tier: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    pub base_seed: u64,
    pub n: u32,
}

/// A board-vs-board matchup (`examples/matchups/*.yaml`).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Matchup {
    pub name: String,
    #[serde(default)]
    pub seed: u64,
    #[serde(default = "default_hero_tier")]
    pub hero_tier_a: u32,
    #[serde(default = "default_hero_tier")]
    pub hero_tier_b: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deity_a: Option<DeityKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deity_b: Option<DeityKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deity_stats_a: Option<[i32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deity_stats_b: Option<[i32; 2]>,
    #[serde(default)]
    pub defaults: Defaults,
    pub team_a: Vec<String>,
    pub team_b: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch: Option<Batch>,
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
        if !apply_unit_token(&mut unit, tok, spec)? {
            return Err(format!("unknown token {tok:?} in {spec:?}"));
        }
    }

    crate::cards::check_stat_thresholds(&mut unit);
    Ok(unit)
}

/// Apply one [`parse_unit`] token (keyword, `golden`, tribe, `card:<slug>`, `tier:<n>` / `t<n>`)
/// to `unit`. Returns `Ok(false)` if `tok` is not such a token.
pub(crate) fn apply_unit_token(unit: &mut Unit, tok: &str, spec: &str) -> Result<bool, String> {
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
        // Card template lookup by slug
        other if other.starts_with("card:") => {
            let slug = &other["card:".len()..];
            let tpl = crate::cards::full_catalog()
                .into_iter()
                .find(|t| {
                    let t_slug = t.name.to_ascii_lowercase().replace([' ', '-', '\''], "_");
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
        other if other.starts_with('t') && other.len() > 1 => match other[1..].parse() {
            Ok(tier) => unit.tavern_tier = tier,
            Err(_) => return Ok(false),
        },
        _ => return Ok(false),
    }
    Ok(true)
}

/// Build `(board_a, board_b, game_state)` from a parsed [`Matchup`].
pub fn teams_and_state(matchup: &Matchup) -> Result<(Vec<Unit>, Vec<Unit>, GameState), String> {
    let board_a: Vec<Unit> = matchup
        .team_a
        .iter()
        .map(|s| parse_unit(s, &matchup.defaults))
        .collect::<Result<_, _>>()?;
    let board_b: Vec<Unit> = matchup
        .team_b
        .iter()
        .map(|s| parse_unit(s, &matchup.defaults))
        .collect::<Result<_, _>>()?;

    let mut state = GameState {
        hero_tier_a: matchup.hero_tier_a,
        hero_tier_b: matchup.hero_tier_b,
        ..GameState::default()
    };
    if let Some(kind) = matchup.deity_a {
        state.auras_a.deity.kind = kind;
    }
    if let Some(kind) = matchup.deity_b {
        state.auras_b.deity.kind = kind;
    }
    if let Some([atk, hp]) = matchup.deity_stats_a {
        state.auras_a.deity.attack = atk;
        state.auras_a.deity.health = hp;
    }
    if let Some([atk, hp]) = matchup.deity_stats_b {
        state.auras_b.deity.attack = atk;
        state.auras_b.deity.health = hp;
    }

    Ok((board_a, board_b, state))
}
