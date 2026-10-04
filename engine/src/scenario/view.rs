//! The view of engine state that expectations match against (`docs/scenarios.md` §5).
//!
//! A [`View`] is the serde form of a value (a `TavernState`, `BattleResult`, ...) with a few
//! derived keys added:
//!
//! - units also have `stats` (`"A/H"`), `keywords` (the keywords they have), `card` (their
//!   card's name) and `battlecry` / `deathrattle` / `rally` / `choose_one` (card flags);
//! - player auras also have `counters` (`card_counters` keyed by card name, `[0, 0]` for cards
//!   without one) and `effect_stacks` (total stacks of each card's player effects, 0 for
//!   cards without one); each effect also has `card`.
//!
//! Enum variants with data (YAML tags such as `!Refreshes 3`) become single-key maps
//! (`{Refreshes: 3}`).

use serde::Serialize;
use serde_yaml::{Mapping, Value};

use crate::cards;
use crate::model::{CardId, PlayerAuras, Unit};

use super::patch::key_name;
use super::units::{self, card_index, KEYWORDS};

/// A value to match expectations against.
#[derive(Clone, Debug, PartialEq)]
pub enum View {
    Null,
    Bool(bool),
    Int(i128),
    Float(f64),
    Str(String),
    List(Vec<View>),
    Map(Vec<(String, View)>),
    /// A unit and its fields (serialized and derived).
    Unit(Box<Unit>, Vec<(String, View)>),
    /// Values keyed by card name (`pool`, `counters`, ...), and the value of cards without an
    /// entry (if they have one).
    CardMap(Vec<(String, View)>, Option<Box<View>>),
}

impl View {
    /// The view of a serializable engine value.
    pub fn of(value: &impl Serialize) -> View {
        from_value(&serde_yaml::to_value(value).expect("engine values serialize to YAML"))
    }

    /// The field `key` of a map or unit.
    pub fn field(&self, key: &str) -> Option<&View> {
        match self {
            View::Map(fields) | View::Unit(_, fields) => {
                fields.iter().find(|(k, _)| k == key).map(|(_, v)| v)
            }
            _ => None,
        }
    }

    /// The field names of a map or unit (the card names of a card map).
    pub fn keys(&self) -> Vec<&str> {
        match self {
            View::Map(fields) | View::Unit(_, fields) | View::CardMap(fields, _) => {
                fields.iter().map(|(k, _)| k.as_str()).collect()
            }
            _ => Vec::new(),
        }
    }

    /// Add a field to a map or unit.
    pub fn push(&mut self, key: &str, value: View) {
        if let View::Map(fields) | View::Unit(_, fields) = self {
            fields.push((key.to_string(), value));
        }
    }

    /// What kind of value this is (for error messages).
    pub fn kind(&self) -> &'static str {
        match self {
            View::Null => "null",
            View::Bool(_) => "a bool",
            View::Int(_) | View::Float(_) => "a number",
            View::Str(_) => "a string",
            View::List(_) => "a list",
            View::Map(_) => "a map",
            View::Unit(..) => "a unit",
            View::CardMap(..) => "a map of cards",
        }
    }

    /// A one-line rendering, truncated to about 200 characters.
    pub fn short(&self) -> String {
        let mut out = String::new();
        self.render(&mut out);
        truncate(out, 200)
    }

    fn render(&self, out: &mut String) {
        match self {
            View::Null => out.push_str("null"),
            View::Bool(b) => out.push_str(&b.to_string()),
            View::Int(n) => out.push_str(&n.to_string()),
            View::Float(f) => out.push_str(&f.to_string()),
            View::Str(s) => out.push_str(&quote(s)),
            View::List(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    item.render(out);
                    if out.len() > 400 {
                        out.push_str(", ...");
                        break;
                    }
                }
                out.push(']');
            }
            View::Map(fields) | View::CardMap(fields, _) => {
                out.push('{');
                for (i, (key, value)) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(key);
                    out.push_str(": ");
                    value.render(out);
                    if out.len() > 400 {
                        out.push_str(", ...");
                        break;
                    }
                }
                out.push('}');
            }
            View::Unit(unit, _) => out.push_str(&units::describe(unit)),
        }
    }
}

fn from_value(value: &Value) -> View {
    match value {
        Value::Null => View::Null,
        Value::Bool(b) => View::Bool(*b),
        Value::Number(n) => match (n.as_i64(), n.as_u64()) {
            (Some(i), _) => View::Int(i.into()),
            (_, Some(u)) => View::Int(u.into()),
            _ => View::Float(n.as_f64().unwrap_or(f64::NAN)),
        },
        Value::String(s) => View::Str(s.clone()),
        Value::Sequence(items) => View::List(items.iter().map(from_value).collect()),
        Value::Mapping(m) if m.contains_key("card_id") && m.contains_key("stitched_stored") => {
            unit_view(value, m)
        }
        Value::Mapping(m) if m.contains_key("card_counters") && m.contains_key("effects") => {
            auras_view(value, m)
        }
        Value::Mapping(m) => View::Map(fields(m)),
        Value::Tagged(tagged) => View::Map(vec![(
            tagged.tag.to_string().trim_start_matches('!').to_string(),
            from_value(&tagged.value),
        )]),
    }
}

fn fields(m: &Mapping) -> Vec<(String, View)> {
    m.iter()
        .map(|(k, v)| (key_name(k), from_value(v)))
        .collect()
}

fn unit_view(value: &Value, m: &Mapping) -> View {
    let unit: Unit = serde_yaml::from_value(value.clone()).expect("units round-trip through serde");
    let mut view = fields(m);
    let id = unit.card_id;
    let keywords = KEYWORDS
        .iter()
        .filter(|&&kw| unit.has_keyword(kw))
        .map(|&kw| View::Str(units::keyword_name(kw)))
        .collect();
    view.extend([
        (
            "stats".to_string(),
            View::Str(format!("{}/{}", unit.attack, unit.health)),
        ),
        ("keywords".to_string(), View::List(keywords)),
        (
            "card".to_string(),
            card_index()
                .by_id(id)
                .map_or(View::Null, |card| View::Str(card.name.clone())),
        ),
        (
            "battlecry".to_string(),
            View::Bool(cards::is_battlecry_minion(id)),
        ),
        (
            "deathrattle".to_string(),
            View::Bool(cards::is_deathrattle_minion(id)),
        ),
        ("rally".to_string(), View::Bool(cards::is_rally_minion(id))),
        (
            "choose_one".to_string(),
            View::Bool(cards::is_choose_one_minion(id)),
        ),
    ]);
    View::Unit(Box::new(unit), view)
}

fn auras_view(value: &Value, m: &Mapping) -> View {
    let auras: PlayerAuras =
        serde_yaml::from_value(value.clone()).expect("player auras round-trip through serde");
    let mut view = fields(m);
    if let Some((_, View::List(effects))) = view.iter_mut().find(|(k, _)| k == "effects") {
        for (effect, data) in effects.iter_mut().zip(&auras.effects) {
            effect.push("card", View::Str(card_name(data.card_id)));
        }
    }
    let counters = auras
        .card_counters
        .iter()
        .map(|(&id, &(a, b))| {
            (
                card_name(id),
                View::List(vec![View::Int(a.into()), View::Int(b.into())]),
            )
        })
        .collect();
    let mut stacks: Vec<(String, View)> = Vec::new();
    for effect in &auras.effects {
        let name = card_name(effect.card_id);
        match stacks.iter_mut().find(|(k, _)| *k == name) {
            Some((_, View::Int(n))) => *n += i128::from(effect.stacks),
            _ => stacks.push((name, View::Int(effect.stacks.into()))),
        }
    }
    view.push((
        "counters".to_string(),
        View::CardMap(
            counters,
            Some(Box::new(View::List(vec![View::Int(0), View::Int(0)]))),
        ),
    ));
    view.push((
        "effect_stacks".to_string(),
        View::CardMap(stacks, Some(Box::new(View::Int(0)))),
    ));
    View::Map(view)
}

/// The name of the card `card_id` (its id, for ids that are not cards).
pub fn card_name(card_id: CardId) -> String {
    card_index()
        .by_id(card_id)
        .map_or_else(|| card_id.to_string(), |card| card.name.clone())
}

/// A compact one-line rendering of a YAML value (flow style, truncated to about 200
/// characters).
pub fn show(value: &Value) -> String {
    let mut out = String::new();
    show_into(value, &mut out);
    truncate(out, 200)
}

fn show_into(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(&b.to_string()),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => out.push_str(&quote(s)),
        Value::Sequence(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                show_into(item, out);
            }
            out.push(']');
        }
        Value::Mapping(m) => {
            out.push('{');
            for (i, (key, item)) in m.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                show_into(key, out);
                out.push_str(": ");
                show_into(item, out);
            }
            out.push('}');
        }
        Value::Tagged(tagged) => {
            out.push_str(&tagged.tag.to_string());
            out.push(' ');
            show_into(&tagged.value, out);
        }
    }
}

/// `s`, quoted if it would not read back as the same plain string.
fn quote(s: &str) -> String {
    let plain = !s.is_empty()
        && s.trim() == s
        && !s.contains(": ")
        && !s.contains(", ")
        && !s.starts_with([
            '{', '[', '"', '\'', '!', '&', '*', '#', '-', '@', '$', '>', '<', '=',
        ])
        && serde_yaml::from_str::<Value>(s).is_ok_and(|v| v.as_str() == Some(s));
    if plain {
        s.to_string()
    } else {
        format!("{s:?}")
    }
}

fn truncate(mut s: String, max: usize) -> String {
    if s.len() > max {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
        s.push_str("...");
    }
    s
}
