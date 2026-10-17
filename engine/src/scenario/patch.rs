//! Field patches (`docs/scenarios.md` §4): set some fields of a value, keep the others.
//!
//! A patch is a YAML mapping of field names to new values, applied to the value's serde form:
//!
//! - a mapping patches a nested struct (`auras: {deity: {kind: yshaarj}}`) or, given indices,
//!   elements of a list (`board: {0: {attack: 5}}`; negative indices count from the end);
//! - anything else replaces the field (`gold: 10`, `hero_health: null`).
//!
//! Unknown fields are errors. Fields holding units (`board`, `hand`, `shop`, `discover_pending`,
//! ...) take unit specs instead of serialized units (`hand: [Joyous, "golden Zoatroid"]`,
//! `board: {0: "Joyous 5/5"}`). A [`PlayerAuras`](crate::model::PlayerAuras) patch also takes
//! `counters: {<card>: [a, b] | n | null}` (its `card_counters`, keyed by card name) and effects
//! given as `{card: <name>, stacks: n, duration: ...}` (`stacks` defaults to 1, `duration` to
//! `Game`), `hero: <name>` (a fresh hero, `docs/heroes.md` §2.2; `{hero: <name>, power: {...}}`
//! patches its fields), and `trinkets: [<name>, {trinket: <name>, counter: n}]` (fresh trinkets,
//! `docs/trinkets.md` §8.3). Enum variants with data are written as in the view: `duration: {Refreshes:
//! 3}`.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_yaml::value::{Tag, TaggedValue};
use serde_yaml::{Mapping, Value};

use super::units::{self, card_index};
use super::view::show;

/// Fields that hold a list of units: patches give them as unit specs.
const UNIT_LISTS: [&str; 11] = [
    "board",
    "hand",
    "shop",
    "discover_pending",
    "stitched_stored",
    "hand_a",
    "hand_b",
    "waiting_for_hand",
    "stored",
    "lobby_gifts",
    "last_warband",
];

/// Fields that hold a list of lists of units (`discover_queue`, a Duos team's `incoming`
/// queues): patches give each list as unit specs. `incoming` also takes `{<player>: [...]}`.
const UNIT_LIST_LISTS: [&str; 2] = ["discover_queue", "incoming"];

/// Unit fields serde leaves out while they have their default (`skip_serializing_if`): a patch
/// can still set them.
const SKIPPED_UNIT_FIELDS: [&str; 9] = [
    "dark_gifts",
    "marked",
    "extra_deathrattles",
    "trinket_marks",
    "temp_reborn",
    "until_next_turn_atk",
    "hats",
    "frozen",
    "to_hero_power",
];

/// `PlayerAuras` fields serde leaves out while they are 0: a patch can still set them.
const SKIPPED_AURAS_FIELDS: [&str; 2] = ["next_minion_discount", "battlecries_triggered"];

/// `TavernState` unit lists serde leaves out while they are empty: a patch can still set them.
const SKIPPED_TAVERN_UNIT_LISTS: [&str; 2] = ["waiting_for_hand", "lobby_gifts"];

/// Patch `target` with `patch`, a mapping of field names to new values.
pub fn apply<T: Serialize + DeserializeOwned>(target: &mut T, patch: &Value) -> Result<(), String> {
    if !patch.is_mapping() {
        return Err(format!(
            "a patch is a mapping of field names to values, got {}",
            show(patch)
        ));
    }
    let mut value = serde_yaml::to_value(&*target).map_err(|e| e.to_string())?;
    merge(&mut value, patch, "")?;
    tag_variants(&mut value);
    *target = serde_yaml::from_value(value)
        .map_err(|e| format!("invalid value in patch {}: {e}", show(patch)))?;
    Ok(())
}

/// Turn each `{Variant: value}` (a single key starting with an uppercase letter; engine field
/// names are snake_case) into the YAML tag `!Variant value`, which is how serde_yaml reads an
/// enum variant with data.
fn tag_variants(value: &mut Value) {
    let tagged = match value {
        Value::Sequence(items) => {
            items.iter_mut().for_each(tag_variants);
            None
        }
        Value::Mapping(fields) => {
            fields.iter_mut().for_each(|(_, v)| tag_variants(v));
            match fields.iter().next() {
                Some((Value::String(variant), data))
                    if fields.len() == 1
                        && variant.starts_with(|c: char| c.is_ascii_uppercase()) =>
                {
                    Some(Value::Tagged(Box::new(TaggedValue {
                        tag: Tag::new(variant.clone()),
                        value: data.clone(),
                    })))
                }
                _ => None,
            }
        }
        Value::Tagged(tagged) => {
            tag_variants(&mut tagged.value);
            None
        }
        _ => None,
    };
    if let Some(tagged) = tagged {
        *value = tagged;
    }
}

fn merge(base: &mut Value, patch: &Value, path: &str) -> Result<(), String> {
    match (base, patch) {
        (Value::Mapping(fields), Value::Mapping(changes)) => merge_fields(fields, changes, path),
        (Value::Sequence(items), Value::Mapping(changes)) => {
            for (key, change) in changes {
                let i = index(key, items.len(), path)?;
                merge(&mut items[i], change, &format!("{path}[{}]", show(key)))?;
            }
            Ok(())
        }
        (base, patch) => {
            *base = patch.clone();
            Ok(())
        }
    }
}

fn merge_fields(fields: &mut Mapping, changes: &Mapping, path: &str) -> Result<(), String> {
    let is_auras = fields.contains_key("card_counters") && fields.contains_key("effects");
    let is_unit = fields.contains_key("card_id") && fields.contains_key("is_spell");
    // A map field (`card_counters`) takes any key; a struct only takes its own fields.
    let open = fields.is_empty() || fields.keys().any(|k| !k.is_string());
    if fields.contains_key("discover_pending") && fields.contains_key("tavern_tier") {
        for name in SKIPPED_TAVERN_UNIT_LISTS {
            if !fields.contains_key(name) {
                fields.insert(Value::from(name), Value::Sequence(Vec::new()));
            }
        }
        if !fields.contains_key("trinkets_enabled") {
            fields.insert(Value::from("trinkets_enabled"), Value::Bool(false));
        }
        if !fields.contains_key("trinket_options") {
            fields.insert(Value::from("trinket_options"), Value::Sequence(Vec::new()));
        }
        if !fields.contains_key("trinket_offer_queue") {
            fields.insert(
                Value::from("trinket_offer_queue"),
                Value::Sequence(Vec::new()),
            );
        }
        for name in ["dark_gifts_enabled", "dark_discovery_used_this_turn"] {
            if !fields.contains_key(name) {
                fields.insert(Value::from(name), Value::Bool(false));
            }
        }
        if !fields.contains_key("dark_discoveries_used") {
            fields.insert(Value::from("dark_discoveries_used"), Value::from(0));
        }
        if !fields.contains_key("lobby") {
            let lobby = serde_yaml::to_value(crate::model::LobbyView::default())
                .map_err(|e| e.to_string())?;
            fields.insert(Value::from("lobby"), lobby);
        }
    }
    // A Hero Power's kept cards, left out while there are none.
    if fields.contains_key("uses_left")
        && fields.contains_key("armed")
        && !fields.contains_key("stored")
    {
        fields.insert(Value::from("stored"), Value::Sequence(Vec::new()));
    }
    // A trinket's kept cards and minion type, left out while there are none.
    if fields.contains_key("stacks")
        && fields.contains_key("armed")
        && !fields.contains_key("uses_left")
    {
        if !fields.contains_key("stored") {
            fields.insert(Value::from("stored"), Value::Sequence(Vec::new()));
        }
        if !fields.contains_key("tribe") {
            fields.insert(Value::from("tribe"), Value::from("none"));
        }
    }
    // A hero's second Hero Power, left out while there is none.
    if fields.contains_key("hero_id") && fields.contains_key("power") {
        let needs_default = changes.get("second_power").is_some_and(Value::is_mapping)
            && fields.get("second_power").is_none_or(Value::is_null);
        if needs_default {
            let sp = serde_yaml::to_value(crate::model::HeroPowerState::default())
                .map_err(|e| e.to_string())?;
            fields.insert(Value::from("second_power"), sp);
        } else if !fields.contains_key("second_power") {
            fields.insert(Value::from("second_power"), Value::Null);
        }
    }
    for (key, change) in changes {
        let name = key_name(key);
        let field_path = join(path, &name);
        if is_auras && name == "counters" {
            set_counters(fields, change, &field_path)?;
            continue;
        }
        if is_auras && name == "hero" {
            set_hero(fields, change, &field_path)?;
            continue;
        }
        if is_auras && name == "trinkets" {
            set_trinkets(fields, change, &field_path)?;
            continue;
        }
        if open && change.is_null() {
            fields.remove(key);
            continue;
        }
        let resolved;
        let change = if is_auras && name == "effects" {
            resolved = effects_by_card(change, &field_path)?;
            &resolved
        } else {
            change
        };
        match fields.get_mut(key) {
            Some(slot) if UNIT_LISTS.contains(&name.as_str()) => {
                patch_units(slot, change, &field_path)?
            }
            Some(slot) if UNIT_LIST_LISTS.contains(&name.as_str()) && change.is_sequence() => {
                let lists = change.as_sequence().into_iter().flatten();
                let lists: Vec<Value> = lists
                    .map(|list| units_value(list, &field_path))
                    .collect::<Result<_, _>>()?;
                *slot = Value::Sequence(lists);
            }
            Some(Value::Sequence(items)) if name == "incoming" && change.is_mapping() => {
                for (key, list) in change.as_mapping().into_iter().flatten() {
                    let i = index(key, items.len(), &field_path)?;
                    items[i] = units_value(list, &format!("{field_path}[{}]", show(key)))?;
                }
            }
            Some(slot) if name == "opponents" && change.is_sequence() => {
                let mut opps = Vec::new();
                for (i, item) in change.as_sequence().into_iter().flatten().enumerate() {
                    let mut opp = serde_yaml::to_value(crate::model::OpponentView::default())
                        .map_err(|e| e.to_string())?;
                    merge(&mut opp, item, &format!("{field_path}[{i}]"))?;
                    opps.push(opp);
                }
                *slot = Value::Sequence(opps);
            }
            Some(slot) => merge(slot, change, &field_path)?,
            None if open
                || (is_unit && SKIPPED_UNIT_FIELDS.contains(&name.as_str()))
                || (is_auras && SKIPPED_AURAS_FIELDS.contains(&name.as_str())) =>
            {
                fields.insert(key.clone(), change.clone());
            }
            None => return Err(unknown_field(&field_path, &name, fields)),
        }
    }
    Ok(())
}

/// Patch a unit-list field: a list of unit specs replaces it, `{<index>: ...}` replaces (unit
/// spec) or patches (mapping of fields) single units, `null` sets it to null.
fn patch_units(slot: &mut Value, change: &Value, path: &str) -> Result<(), String> {
    match change {
        Value::Null => *slot = Value::Null,
        Value::Mapping(changes) if !is_unit_spec(change) => {
            let Value::Sequence(items) = slot else {
                return Err(format!(
                    "`{path}` is {}: it has no units to patch",
                    show(slot)
                ));
            };
            for (key, unit_change) in changes {
                let i = index(key, items.len(), path)?;
                let item_path = format!("{path}[{}]", show(key));
                if is_unit_spec(unit_change) {
                    items[i] = unit_value(unit_change, &item_path)?;
                } else if unit_change.is_mapping() {
                    merge(&mut items[i], unit_change, &item_path)?;
                } else {
                    return Err(format!(
                        "`{item_path}`: expected a unit spec or a mapping of unit fields, got {}",
                        show(unit_change)
                    ));
                }
            }
        }
        _ => *slot = units_value(change, path)?,
    }
    Ok(())
}

/// A unit spec: a string, or a mapping with a `card` key.
fn is_unit_spec(value: &Value) -> bool {
    value.is_string() || value.as_mapping().is_some_and(|m| m.contains_key("card"))
}

fn unit_value(spec: &Value, path: &str) -> Result<Value, String> {
    let unit = units::build(spec).map_err(|e| format!("`{path}`: {e}"))?;
    serde_yaml::to_value(unit).map_err(|e| e.to_string())
}

fn units_value(specs: &Value, path: &str) -> Result<Value, String> {
    let units = units::build_list(specs).map_err(|e| format!("`{path}`: {e}"))?;
    serde_yaml::to_value(units).map_err(|e| e.to_string())
}

/// `counters: {<card>: [a, b] | n | null}`: set (`n` means `[n, 0]`) or remove card counters.
fn set_counters(fields: &mut Mapping, change: &Value, path: &str) -> Result<(), String> {
    let Value::Mapping(changes) = change else {
        return Err(format!(
            "`{path}`: expected a mapping of card names to counters, got {}",
            show(change)
        ));
    };
    let counters = fields
        .get_mut("card_counters")
        .and_then(Value::as_mapping_mut)
        .ok_or_else(|| format!("`{path}`: no card_counters to patch"))?;
    for (key, value) in changes {
        let card_id = card_id_of(key).map_err(|e| format!("`{path}`: {e}"))?;
        let id = serde_yaml::to_value(card_id).map_err(|e| e.to_string())?;
        match value {
            Value::Null => {
                counters.remove(&id);
            }
            Value::Number(_) => {
                counters.insert(id, Value::Sequence(vec![value.clone(), Value::from(0)]));
            }
            Value::Sequence(pair) if pair.len() == 2 => {
                counters.insert(id, value.clone());
            }
            other => {
                return Err(format!(
                    "`{path}.{}`: expected [a, b], a number or null, got {}",
                    show(key),
                    show(other)
                ))
            }
        }
    }
    Ok(())
}

/// `hero: <name> | {hero: <name>, <field>: ...} | null`: the player's hero
/// (`docs/heroes.md` §2.2), fresh (`heroes::new_state`), with the mapping's other keys patching
/// its fields (`power: {armed: 1}`), or none.
fn set_hero(fields: &mut Mapping, change: &Value, path: &str) -> Result<(), String> {
    let (name, rest) = match change {
        Value::Null => {
            fields.remove("hero");
            return Ok(());
        }
        Value::String(name) => (name.as_str(), None),
        Value::Mapping(m) => match m.get("hero").and_then(Value::as_str) {
            Some(name) => {
                let mut rest = m.clone();
                rest.remove("hero");
                (name, Some(Value::Mapping(rest)))
            }
            None => {
                // A patch of the current hero's fields.
                let slot = fields
                    .get_mut("hero")
                    .ok_or_else(|| format!("`{path}`: no hero to patch (give `hero: <name>`)"))?;
                return merge(slot, change, path);
            }
        },
        other => {
            return Err(format!(
                "`{path}`: expected a hero name, {{hero: <name>, ...}} or null, got {}",
                show(other)
            ))
        }
    };
    let id = super::spec::hero_id(name).map_err(|e| format!("`{path}`: {e}"))?;
    let hero = crate::heroes::new_state(crate::heroes::data(id).expect("a hero"));
    let mut value = serde_yaml::to_value(hero).map_err(|e| e.to_string())?;
    if let Some(rest) = rest {
        merge(&mut value, &rest, path)?;
    }
    fields.insert("hero".into(), value);
    Ok(())
}

/// `trinkets: [<name> | {trinket: <name>, <field>: ...}]`: the player's trinkets
/// (`docs/trinkets.md` §8.3), fresh (`trinkets::new_state`), with a mapping's other keys patching
/// their fields (`counter: 3`). The list replaces the player's trinkets.
fn set_trinkets(fields: &mut Mapping, change: &Value, path: &str) -> Result<(), String> {
    let items = match change {
        Value::Null => Vec::new(),
        Value::Sequence(items) => items.clone(),
        other => vec![other.clone()],
    };
    let mut trinkets = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let item_path = format!("{path}[{i}]");
        let (name, rest) = match item {
            Value::String(name) => (name.as_str(), None),
            Value::Mapping(m) => match m.get("trinket").and_then(Value::as_str) {
                Some(name) => {
                    let mut rest = m.clone();
                    rest.remove("trinket");
                    (name, Some(Value::Mapping(rest)))
                }
                None => return Err(format!("`{item_path}`: expected {{trinket: <name>, ...}}")),
            },
            other => {
                return Err(format!(
                    "`{item_path}`: expected a trinket name or {{trinket: <name>, ...}}, got {}",
                    show(other)
                ))
            }
        };
        let id = super::spec::trinket_id(name).map_err(|e| format!("`{item_path}`: {e}"))?;
        let mut value =
            serde_yaml::to_value(crate::trinkets::new_state(id)).map_err(|e| e.to_string())?;
        if let Some(rest) = rest {
            merge(&mut value, &rest, &item_path)?;
        }
        trinkets.push(value);
    }
    fields.insert("trinkets".into(), Value::Sequence(trinkets));
    Ok(())
}

/// Effects given with `card: <name>` instead of `card_id` (as a list, or as `{<index>: ...}`).
fn effects_by_card(change: &Value, path: &str) -> Result<Value, String> {
    let resolve = |effect: &Value| -> Result<Value, String> {
        let Some(fields) = effect.as_mapping().filter(|m| m.contains_key("card")) else {
            return Ok(effect.clone());
        };
        let mut fields = fields.clone();
        let card = fields.remove("card").unwrap_or_default();
        let card_id = card_id_of(&card).map_err(|e| format!("`{path}`: {e}"))?;
        fields.insert(
            "card_id".into(),
            serde_yaml::to_value(card_id).map_err(|e| e.to_string())?,
        );
        if !fields.contains_key("stacks") {
            fields.insert("stacks".into(), Value::from(1));
        }
        if !fields.contains_key("duration") {
            fields.insert("duration".into(), "Game".into());
        }
        Ok(Value::Mapping(fields))
    };
    match change {
        Value::Sequence(list) => Ok(Value::Sequence(
            list.iter().map(resolve).collect::<Result<_, _>>()?,
        )),
        Value::Mapping(by_index) if !by_index.contains_key("card") => {
            let mut out = Mapping::new();
            for (key, effect) in by_index {
                out.insert(key.clone(), resolve(effect)?);
            }
            Ok(Value::Mapping(out))
        }
        other => Ok(other.clone()),
    }
}

/// A card id given as a number or a card name.
fn card_id_of(key: &Value) -> Result<u32, String> {
    match key {
        Value::Number(n) => n
            .as_u64()
            .and_then(|id| u32::try_from(id).ok())
            .ok_or_else(|| format!("{n} is not a card id")),
        Value::String(name) => Ok(card_index().resolve(name)?.card_id),
        other => Err(format!("expected a card name or id, got {}", show(other))),
    }
}

/// The index `key` (negative: from the end) into a list of `len` items.
fn index(key: &Value, len: usize, path: &str) -> Result<usize, String> {
    let i = match key {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    };
    let resolved = i.and_then(|i| {
        let i = if i < 0 { len as i64 + i } else { i };
        usize::try_from(i).ok().filter(|&i| i < len)
    });
    resolved.ok_or_else(|| {
        format!(
            "`{path}`: {} is not an index into a list of {len} item(s)",
            show(key)
        )
    })
}

/// A mapping key as a field name.
pub(crate) fn key_name(key: &Value) -> String {
    match key {
        Value::String(s) => s.clone(),
        other => show(other),
    }
}

/// `path.name` (or `name` at the root).
pub(crate) fn join(path: &str, name: &str) -> String {
    if path.is_empty() {
        name.to_string()
    } else {
        format!("{path}.{name}")
    }
}

fn unknown_field(path: &str, name: &str, fields: &Mapping) -> String {
    let names: Vec<String> = fields.keys().map(key_name).collect();
    let close = units::closest(name, names.iter().map(String::as_str));
    if close.is_empty() {
        format!("unknown field `{path}` (fields: {})", names.join(", "))
    } else {
        format!(
            "unknown field `{path}` (did you mean `{}`?)",
            close.join("`, `")
        )
    }
}
