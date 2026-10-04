//! Card names and unit specs (`docs/scenarios.md` §3).
//!
//! A unit spec is a compact string naming a unit:
//!
//! ```text
//! [golden] <Card Name> {token}    "golden Zoatroid +2/+1 taunt"
//! <A>/<H> {token}                 "3/2 divine_shield demon" (a vanilla unit, as in matchups)
//! ```
//!
//! Card names match any minion, spell, token or deity, case-insensitively. Tokens: `A/H` (set
//! the current Attack/Health, like assigning the fields), `+A/+H` (a buff: `Unit::add_stats`),
//! a keyword (`taunt`) or `-keyword`, `golden` / `-golden`, a tribe (`beast`), `tN` (Tavern Tier).
//! An `A/H` spec is built exactly like a matchup unit ([`parse_unit`](super::parse_unit), so
//! `card:<slug>` and `tier:<n>` work there too).
//!
//! In expectations the same string is a matcher: it checks the card, the stats, `golden`, the
//! listed keywords (present, or absent for `-keyword`), the tribe (`Tribe::matches`) and the
//! tier, and nothing else.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde_yaml::Value;

use crate::cards::{self, spells, tokens};
use crate::model::{CardId, DeityKind, DeityState, Keyword, Tribe, Unit};

use super::{matchup, patch, Failure};

/// What a card name refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardKind {
    Minion,
    Spell,
    Token,
    Deity,
}

/// A card that unit specs can name.
#[derive(Debug)]
pub struct Card {
    pub name: String,
    pub card_id: CardId,
    pub kind: CardKind,
    /// A plain instance of the card: what a spec naming it starts from.
    pub proto: Unit,
}

/// Every card that unit specs can name: catalog minions, spells (Tavern and token spells),
/// token minions and the deities.
pub struct CardIndex {
    cards: Vec<Card>,
    /// Lowercase name -> indices into `cards` (more than one: an ambiguous name).
    by_name: BTreeMap<String, Vec<usize>>,
    by_id: BTreeMap<CardId, usize>,
}

/// The card index (built once).
pub fn card_index() -> &'static CardIndex {
    static INDEX: OnceLock<CardIndex> = OnceLock::new();
    INDEX.get_or_init(CardIndex::build)
}

impl CardIndex {
    fn build() -> Self {
        let mut list = Vec::new();
        for tpl in cards::all_templates() {
            list.push(Card {
                name: tpl.name.clone(),
                card_id: tpl.card_id,
                kind: CardKind::Minion,
                proto: tpl.instantiate(),
            });
        }
        let others = [
            (CardKind::Spell, spells::all_spells()),
            (CardKind::Token, tokens::all_token_minions()),
            (
                CardKind::Deity,
                [DeityKind::CThun, DeityKind::YShaarj]
                    .map(|kind| cards::instantiate_deity(&DeityState::new(kind)))
                    .to_vec(),
            ),
        ];
        for (kind, units) in others {
            for unit in units {
                list.push(Card {
                    name: unit.name.clone(),
                    card_id: unit.card_id,
                    kind,
                    proto: unit,
                });
            }
        }

        let mut by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut by_id = BTreeMap::new();
        for (i, card) in list.iter().enumerate() {
            by_name
                .entry(card.name.to_ascii_lowercase())
                .or_default()
                .push(i);
            by_id.entry(card.card_id).or_insert(i);
        }
        Self {
            cards: list,
            by_name,
            by_id,
        }
    }

    /// Every card, in index order (minions in catalog order, then spells, tokens, deities).
    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    /// The card named `name` (case-insensitive).
    pub fn resolve(&self, name: &str) -> Result<&Card, String> {
        let key = name.trim().to_ascii_lowercase();
        match self.by_name.get(&key).map(Vec::as_slice) {
            Some([i]) => Ok(&self.cards[*i]),
            Some(many) => Err(format!(
                "card name {name:?} is ambiguous (card ids {})",
                many.iter()
                    .map(|&i| self.cards[i].card_id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            None => Err(format!("unknown card {name:?}{}", self.suggest(&key))),
        }
    }

    /// The card with id `card_id`.
    pub fn by_id(&self, card_id: CardId) -> Option<&Card> {
        self.by_id.get(&card_id).map(|&i| &self.cards[i])
    }

    /// The longest card name `spec` starts with (followed by whitespace or the end), and the
    /// rest of `spec`.
    fn prefix<'s>(&self, spec: &'s str) -> Result<Option<(&Card, &'s str)>, String> {
        let lower = spec.to_ascii_lowercase();
        let best = self
            .by_name
            .iter()
            .filter(|(name, _)| {
                lower.starts_with(name.as_str())
                    && lower[name.len()..]
                        .chars()
                        .next()
                        .is_none_or(char::is_whitespace)
            })
            .max_by_key(|(name, _)| name.len());
        match best {
            None => Ok(None),
            Some((name, idx)) if idx.len() == 1 => {
                Ok(Some((&self.cards[idx[0]], &spec[name.len()..])))
            }
            Some((name, _)) => Err(format!("card name {name:?} is ambiguous")),
        }
    }

    /// `" (did you mean ...?)"` for the card names closest to `key`, or nothing.
    fn suggest(&self, key: &str) -> String {
        let mut close: Vec<(usize, &str)> = self
            .by_name
            .iter()
            .filter_map(|(name, idx)| {
                let distance = levenshtein(key, name);
                let near = distance <= (key.len() / 3).max(2);
                let contains =
                    key.len() >= 3 && (name.contains(key) || key.contains(name.as_str()));
                (near || contains).then(|| (distance, self.cards[idx[0]].name.as_str()))
            })
            .collect();
        close.sort();
        close.truncate(3);
        if close.is_empty() {
            return String::new();
        }
        let names: Vec<String> = close.iter().map(|(_, n)| format!("{n:?}")).collect();
        format!(" (did you mean {}?)", names.join(", "))
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

/// The (up to 3) names among `candidates` closest to `name`, best first (for "did you mean"
/// hints).
pub(crate) fn closest<'a>(
    name: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Vec<&'a str> {
    let mut close: Vec<(usize, &str)> = candidates
        .into_iter()
        .filter_map(|c| {
            let distance = levenshtein(name, c);
            let near = distance <= (name.len() / 3).max(2);
            let contains = name.len() >= 3 && (c.contains(name) || name.contains(c));
            (near || contains).then_some((distance, c))
        })
        .collect();
    close.sort();
    close.dedup();
    close.truncate(3);
    close.into_iter().map(|(_, c)| c).collect()
}

/// The file-name slug of a card name: its lowercase ASCII words joined by `_`, apostrophes
/// dropped (`"Fandral's Fortune"` -> `fandrals_fortune`, `"Accord-o-Tron"` -> `accord_o_tron`).
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in name.chars().filter(|&c| c != '\'') {
        if c.is_ascii_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('_');
            }
            gap = false;
            out.push(c.to_ascii_lowercase());
        } else {
            gap = true;
        }
    }
    out
}

/// How a unit spec begins.
enum Head {
    Card(&'static Card),
    /// `A/H`: a vanilla unit.
    Stats(i32, i32),
    /// Tokens only (expectations).
    Bare,
}

/// One unit spec token after the head.
enum Mod {
    Golden(bool),
    Stats(i32, i32),
    Buff(i32, i32),
    Keyword(Keyword, bool),
    Tribe(Tribe),
    Tier(u32),
}

/// Split `spec` into its head, whether it starts with `golden`, and the remaining tokens.
fn split_head(spec: &str) -> Result<(Head, bool, &str), String> {
    let index = card_index();
    let spec = spec.trim();
    if let Some((card, rest)) = index.prefix(spec)? {
        return Ok((Head::Card(card), false, rest));
    }
    if spec.len() > 7 && spec[..7].eq_ignore_ascii_case("golden ") {
        if let Some((card, rest)) = index.prefix(spec[7..].trim_start())? {
            return Ok((Head::Card(card), true, rest));
        }
    }
    let first = spec.split_whitespace().next().unwrap_or_default();
    if let Some((attack, health)) = parse_stats(first) {
        return Ok((Head::Stats(attack, health), false, &spec[first.len()..]));
    }
    Ok((Head::Bare, false, spec))
}

/// `A/H` with unsigned numbers.
fn parse_stats(tok: &str) -> Option<(i32, i32)> {
    let (a, h) = tok.split_once('/')?;
    let unsigned = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    (unsigned(a) && unsigned(h)).then(|| Some((a.parse().ok()?, h.parse().ok()?)))?
}

/// `+A/+H` (each part signed).
fn parse_buff(tok: &str) -> Option<(i32, i32)> {
    let (a, h) = tok.split_once('/')?;
    let signed = |s: &str| {
        (s.starts_with('+') || s.starts_with('-'))
            && s.len() > 1
            && s[1..].bytes().all(|b| b.is_ascii_digit())
    };
    (signed(a) && signed(h)).then(|| Some((a.parse().ok()?, h.parse().ok()?)))?
}

/// A keyword or tribe by its snake_case name.
fn named<T: serde::de::DeserializeOwned>(name: &str) -> Option<T> {
    serde_yaml::from_value(Value::String(name.to_string())).ok()
}

fn parse_mod(tok: &str) -> Result<Mod, String> {
    let tok = tok.to_ascii_lowercase();
    match tok.as_str() {
        "golden" => return Ok(Mod::Golden(true)),
        "-golden" => return Ok(Mod::Golden(false)),
        _ => {}
    }
    if let Some((a, h)) = parse_stats(&tok) {
        return Ok(Mod::Stats(a, h));
    }
    if let Some((a, h)) = parse_buff(&tok) {
        return Ok(Mod::Buff(a, h));
    }
    let (name, on) = match tok.strip_prefix('-') {
        Some(name) => (name, false),
        None => (tok.as_str(), true),
    };
    if let Some(kw) = named::<Keyword>(name) {
        return Ok(Mod::Keyword(kw, on));
    }
    if on {
        if let Some(tribe) = named::<Tribe>(name) {
            return Ok(Mod::Tribe(tribe));
        }
        let tier = tok.strip_prefix("tier:").or_else(|| tok.strip_prefix('t'));
        if let Some(tier) = tier.and_then(|t| t.parse().ok()) {
            return Ok(Mod::Tier(tier));
        }
    }
    Err(format!("unknown token {tok:?}"))
}

fn remove_keyword(unit: &mut Unit, kw: Keyword) {
    match kw {
        Keyword::Taunt => unit.taunt = false,
        Keyword::DivineShield => {
            unit.divine_shield = false;
            unit.inherent_divine_shield = false;
        }
        Keyword::Windfury => unit.windfury = false,
        Keyword::Reborn => unit.reborn = false,
        Keyword::Venomous => unit.venomous = false,
        Keyword::Stealth => unit.stealth = false,
        Keyword::Magnetic => unit.magnetic = false,
    }
}

/// Build a unit from a spec: a spec string, or a mapping `{card: <spec>, <Unit field>: value,
/// ...}` whose other keys patch the unit's fields.
pub fn build(spec: &Value) -> Result<Unit, String> {
    match spec {
        Value::String(s) => build_str(s),
        Value::Mapping(m) => {
            let card = m
                .get("card")
                .and_then(Value::as_str)
                .ok_or("a unit given as a mapping needs a `card: <unit spec>` key")?;
            let mut unit = build_str(card)?;
            let mut fields = m.clone();
            fields.remove("card");
            patch::apply(&mut unit, &Value::Mapping(fields))?;
            Ok(unit)
        }
        other => Err(format!(
            "expected a unit spec, got {}",
            super::view::show(other)
        )),
    }
}

/// Build units from a list of specs (or a single spec).
pub fn build_list(specs: &Value) -> Result<Vec<Unit>, String> {
    match specs {
        Value::Sequence(list) => list.iter().map(build).collect(),
        Value::Null => Ok(Vec::new()),
        one => Ok(vec![build(one)?]),
    }
}

fn build_str(spec: &str) -> Result<Unit, String> {
    let (head, golden, rest) = split_head(spec)?;
    let mut unit = match head {
        Head::Card(card) => card.proto.clone(),
        Head::Stats(attack, health) => return build_vanilla(spec, attack, health, rest),
        Head::Bare => {
            return Err(format!(
                "unit spec {spec:?} must start with a card name or `A/H`{}",
                card_index().suggest(&spec.to_ascii_lowercase())
            ))
        }
    };
    unit.is_golden |= golden;
    for tok in rest.split_whitespace() {
        match parse_mod(tok).map_err(|e| format!("{e} in unit spec {spec:?}"))? {
            Mod::Golden(golden) => unit.is_golden = golden,
            Mod::Stats(attack, health) => {
                unit.attack = attack;
                unit.health = health;
            }
            Mod::Buff(attack, health) => unit.add_stats(attack, health),
            Mod::Keyword(kw, true) => unit.apply_keyword(kw, true),
            Mod::Keyword(kw, false) => remove_keyword(&mut unit, kw),
            Mod::Tribe(tribe) => unit.tribe = tribe,
            Mod::Tier(tier) => unit.tavern_tier = tier,
        }
    }
    Ok(unit)
}

/// An `A/H` unit: built like [`parse_unit`](super::parse_unit), plus `-keyword`, `-golden`,
/// `+A/+H` and multi-tribe tokens.
fn build_vanilla(spec: &str, attack: i32, health: i32, rest: &str) -> Result<Unit, String> {
    let mut unit = Unit::new(spec.trim(), attack, health);
    for tok in rest.split_whitespace() {
        if matchup::apply_unit_token(&mut unit, tok, spec)? {
            continue;
        }
        match parse_mod(tok).map_err(|e| format!("{e} in unit spec {spec:?}"))? {
            Mod::Golden(false) => unit.is_golden = false,
            Mod::Buff(a, h) => unit.add_stats(a, h),
            Mod::Keyword(kw, false) => remove_keyword(&mut unit, kw),
            Mod::Tribe(tribe) => unit.tribe = tribe,
            _ => return Err(format!("token {tok:?} is not valid in unit spec {spec:?}")),
        }
    }
    cards::check_stat_thresholds(&mut unit);
    Ok(unit)
}

/// Check `unit` against the unit spec `spec` (an expectation).
pub fn check(spec: &str, unit: &Unit) -> Result<(), Failure> {
    let (head, golden, rest) = split_head(spec).map_err(Failure::Invalid)?;
    let mut wrong = Vec::new();
    match head {
        Head::Card(card) if unit.card_id != card.card_id => {
            wrong.push(format!("is not {}", card.name))
        }
        Head::Stats(attack, health) if (unit.attack, unit.health) != (attack, health) => {
            wrong.push(format!("is not {attack}/{health}"))
        }
        _ => {}
    }
    if golden && !unit.is_golden {
        wrong.push("is not golden".to_string());
    }
    for tok in rest.split_whitespace() {
        let m =
            parse_mod(tok).map_err(|e| Failure::Invalid(format!("{e} in unit spec {spec:?}")))?;
        match m {
            Mod::Golden(want) if unit.is_golden != want => {
                wrong.push(if want { "is not golden" } else { "is golden" }.to_string())
            }
            Mod::Stats(attack, health) if (unit.attack, unit.health) != (attack, health) => {
                wrong.push(format!("is not {attack}/{health}"))
            }
            Mod::Buff(..) => {
                return Err(Failure::Invalid(format!(
                    "`{tok}` (a buff) only works in setup; expect stats with `A/H` in {spec:?}"
                )))
            }
            Mod::Keyword(kw, want) if unit.has_keyword(kw) != want => {
                let name = keyword_name(kw);
                wrong.push(if want {
                    format!("lacks {name}")
                } else {
                    format!("has {name}")
                })
            }
            Mod::Tribe(tribe) if !tribe_matches(unit.tribe, tribe) => {
                wrong.push(format!("is not {}", tribe.as_str()))
            }
            Mod::Tier(tier) if unit.tavern_tier != tier => {
                wrong.push(format!("is not tier {tier}"))
            }
            _ => {}
        }
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(Failure::Mismatch(format!(
            "expected {spec:?}, got {} (it {})",
            describe(unit),
            wrong.join(", ")
        )))
    }
}

/// `want` as a unit spec tribe: `none` means exactly no tribe, others use `Tribe::matches`.
fn tribe_matches(actual: Tribe, want: Tribe) -> bool {
    match want {
        Tribe::None => actual == Tribe::None,
        _ => actual.matches(want),
    }
}

/// Every keyword, in a fixed order.
pub const KEYWORDS: [Keyword; 7] = [
    Keyword::Taunt,
    Keyword::DivineShield,
    Keyword::Windfury,
    Keyword::Reborn,
    Keyword::Venomous,
    Keyword::Stealth,
    Keyword::Magnetic,
];

/// The snake_case name of `kw`.
pub fn keyword_name(kw: Keyword) -> String {
    match serde_yaml::to_value(kw) {
        Ok(Value::String(s)) => s,
        _ => format!("{kw:?}"),
    }
}

/// A one-line description of a unit: `"Golden Zoatroid 6/4 (taunt, golden)"`.
pub fn describe(unit: &Unit) -> String {
    let mut tags: Vec<String> = KEYWORDS
        .iter()
        .filter(|&&kw| unit.has_keyword(kw))
        .map(|&kw| keyword_name(kw))
        .collect();
    if unit.is_golden {
        tags.push("golden".to_string());
    }
    let tags = if tags.is_empty() {
        String::new()
    } else {
        format!(" ({})", tags.join(", "))
    };
    format!("{} {}/{}{tags}", unit.name, unit.attack, unit.health)
}
