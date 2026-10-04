//! Scenario files (`docs/scenarios.md` §2–§4): parsing and validation.
//!
//! ```yaml
//! card: Joyous                # or `topic: <text>` for files about engine mechanics
//! defaults: {seed: 101}       # merged into every scenario (mappings merge, values replace;
//!                             # a scenario's own `combat:` drops a default `tavern:`)
//! scenarios:
//!   - name: battlecry_buffs_the_deity
//!     tavern: {gold: 10}      # a Tavern: `TavernState::new()` patched by this mapping
//!     steps:
//!       - add_to_hand: Joyous
//!       - play: 0
//!     expect: {auras: {deity: {attack: 3, health: 2}}}
//! ```
//!
//! A scenario runs in one of three modes: a Tavern (the default, `tavern:`), a standalone
//! combat (`combat:`, simulated before the steps) or a batch of combats (`combat:` and
//! `batch:`, only `expect:`). A scenario that does not compile still loads; it fails with
//! [`Failure::Invalid`](super::Failure::Invalid) when run.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;
use serde_yaml::{Mapping, Value};

use crate::cards;
use crate::model::{PlayerAuras, Unit};

use super::matchup::Batch;
use super::patch;
use super::units::{self, card_index};
use super::view::show;

/// What a scenario file is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Header {
    /// `card: <name>`: one card (its canonical name).
    Card(String),
    /// `topic: <text>`: an engine mechanic.
    Topic(String),
}

/// A parsed scenario file.
#[derive(Debug)]
pub struct ScenarioFile {
    pub header: Header,
    pub scenarios: Vec<Scenario>,
}

/// One scenario.
#[derive(Debug)]
pub struct Scenario {
    pub name: String,
    pub description: Option<String>,
    /// Set if the scenario reproduces a known engine bug: it is expected to fail (with a
    /// mismatch, not because the scenario is invalid).
    pub known_bug: Option<String>,
    pub(super) body: Result<Body, String>,
}

#[derive(Debug)]
pub(super) struct Body {
    pub seeds: Vec<u64>,
    pub setup: Setup,
    pub steps: Vec<Step>,
}

#[derive(Debug)]
pub(super) enum Setup {
    /// A Tavern: `TavernState::new()` patched by `patch`, with a pool of the catalog `catalog`.
    Tavern { catalog: String, patch: Value },
    /// A standalone combat, simulated before the steps.
    Combat(CombatPlan),
    /// A batch of combats (`simulate_batch`).
    Batch(CombatPlan, Batch),
}

#[derive(Debug)]
pub(super) struct Step {
    /// The step as written (compact), for messages.
    pub desc: String,
    pub kind: StepKind,
}

#[derive(Debug)]
pub(super) enum StepKind {
    StartTurn,
    SyncAllAuras,
    Action(Action),
    AddToHand(Vec<Unit>),
    Push(Zone, Vec<Unit>),
    Clear(Vec<Zone>),
    Set(Value),
    DealHeroDamage(i32),
    ApplyGlobalUnitAuras(Zone, Index),
    Fight(Box<FightPlan>),
    Simulate(Box<CombatPlan>),
    Expect(Value),
    ExpectError(Action),
    ExpectLegal(Action, bool),
    If {
        condition: Value,
        then: Vec<Step>,
        otherwise: Vec<Step>,
    },
    Repeat {
        times: u32,
        steps: Vec<Step>,
    },
}

/// A Tavern zone holding cards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Zone {
    Board,
    Hand,
    Shop,
}

impl Zone {
    fn parse(name: &str) -> Result<Zone, String> {
        match name {
            "board" => Ok(Zone::Board),
            "hand" => Ok(Zone::Hand),
            "shop" => Ok(Zone::Shop),
            other => Err(format!("unknown zone {other:?} (zones: board, hand, shop)")),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Zone::Board => "board",
            Zone::Hand => "hand",
            Zone::Shop => "shop",
        }
    }
}

/// A card position: an index (negative: from the end) or a matcher (the first matching card).
#[derive(Debug)]
pub(super) enum Index {
    At(i64),
    Find(Value),
}

/// A Tavern action (`TavernState::step`).
#[derive(Debug)]
pub(super) enum Action {
    Buy(Index),
    Sell(Index),
    /// `pos` defaults to the end of the board.
    Play {
        hand: Index,
        pos: Option<usize>,
    },
    Reposition {
        from: Index,
        to: usize,
    },
    Activate {
        pos: Index,
        target: Option<usize>,
    },
    Refresh,
    UpgradeTavern,
    ToggleFreeze,
    ChooseDiscover(Index),
    EndTurn,
}

/// Either given in the scenario, or `tavern` (copied from the Tavern when the step runs).
#[derive(Debug)]
pub(super) enum FromTavern<T> {
    Given(T),
    Tavern,
}

/// A standalone combat (`sim::simulate`).
#[derive(Debug)]
pub(super) struct CombatPlan {
    pub seed: Option<u64>,
    pub board_a: FromTavern<Vec<Unit>>,
    pub board_b: FromTavern<Vec<Unit>>,
    pub hand_a: FromTavern<Vec<Unit>>,
    pub hand_b: FromTavern<Vec<Unit>>,
    pub hero_tier_a: u32,
    pub hero_tier_b: u32,
    pub auras_a: FromTavern<PlayerAuras>,
    pub auras_b: FromTavern<PlayerAuras>,
}

/// The Tavern's combat against an opponent (`TavernState::resolve_combat_against`).
#[derive(Debug)]
pub(super) struct FightPlan {
    pub seed: Option<u64>,
    pub board: Vec<Unit>,
    pub tier: u32,
    pub auras: PlayerAuras,
    pub hand: Vec<Unit>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScenario {
    #[allow(dead_code)]
    name: String,
    #[serde(default)]
    #[allow(dead_code)]
    description: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    known_bug: Option<String>,
    #[serde(default)]
    seed: Option<u64>,
    #[serde(default)]
    seeds: Option<Value>,
    #[serde(default)]
    tavern: Option<Value>,
    #[serde(default)]
    combat: Option<Value>,
    #[serde(default)]
    batch: Option<Batch>,
    #[serde(default)]
    steps: Option<Vec<Value>>,
    #[serde(default)]
    expect: Option<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCombat {
    #[serde(default)]
    seed: Option<u64>,
    #[serde(default)]
    board_a: Value,
    #[serde(default)]
    board_b: Value,
    #[serde(default)]
    hand_a: Value,
    #[serde(default)]
    hand_b: Value,
    #[serde(default = "tier_one")]
    hero_tier_a: u32,
    #[serde(default = "tier_one")]
    hero_tier_b: u32,
    #[serde(default)]
    auras_a: Value,
    #[serde(default)]
    auras_b: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFight {
    #[serde(default)]
    seed: Option<u64>,
    #[serde(default)]
    board: Value,
    #[serde(default = "tier_one")]
    tier: u32,
    #[serde(default)]
    auras: Value,
    #[serde(default)]
    hand: Value,
}

fn tier_one() -> u32 {
    1
}

const STEP_NAMES: &str = "start_turn, buy, play, sell, reposition, activate, refresh, \
    upgrade_tavern, toggle_freeze, choose_discover, end_turn, add_to_hand, push_board, \
    push_hand, push_shop, clear, set, deal_hero_damage, apply_global_unit_auras, \
    sync_all_auras, fight, simulate, expect, expect_error, expect_legal, expect_illegal, \
    if/then/else, repeat/steps";

impl ScenarioFile {
    /// Load and validate the scenario file at `path`.
    pub fn load(path: &Path) -> Result<ScenarioFile, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Self::parse(&text)
    }

    /// Parse and validate a scenario file.
    pub fn parse(text: &str) -> Result<ScenarioFile, String> {
        let doc: Value = serde_yaml::from_str(text).map_err(|e| format!("invalid YAML: {e}"))?;
        let Value::Mapping(mut top) = doc else {
            return Err(
                "a scenario file is a mapping (`card:` or `topic:`, `defaults:`, `scenarios:`)"
                    .to_string(),
            );
        };
        for key in top.keys() {
            if !matches!(
                key.as_str(),
                Some("card" | "topic" | "defaults" | "scenarios")
            ) {
                return Err(format!(
                    "unknown top-level key {} (keys: card, topic, defaults, scenarios)",
                    show(key)
                ));
            }
        }
        let header = match (top.get("card"), top.get("topic")) {
            (Some(card), None) => {
                let name = card.as_str().ok_or("`card:` must be a card name")?;
                Header::Card(card_index().resolve(name)?.name.clone())
            }
            (None, Some(topic)) => Header::Topic(
                topic
                    .as_str()
                    .ok_or("`topic:` must be a string")?
                    .to_string(),
            ),
            _ => {
                return Err("a scenario file needs exactly one of `card:` and `topic:`".to_string())
            }
        };
        let defaults = match top.remove("defaults") {
            None | Some(Value::Null) => Mapping::new(),
            Some(Value::Mapping(m)) => m,
            Some(other) => {
                return Err(format!(
                    "`defaults:` must be a mapping, got {}",
                    show(&other)
                ))
            }
        };
        let list = match top.remove("scenarios") {
            Some(Value::Sequence(list)) if !list.is_empty() => list,
            _ => return Err("`scenarios:` must be a non-empty list".to_string()),
        };
        let mut names = BTreeSet::new();
        let mut scenarios = Vec::new();
        for (i, raw) in list.into_iter().enumerate() {
            if !raw.is_mapping() {
                return Err(format!("scenario #{} is not a mapping", i + 1));
            }
            // A scenario's own mode wins: its `combat:` drops the default `tavern:`, and its
            // `tavern:` drops the default `combat:` and `batch:`.
            let mut base = defaults.clone();
            if raw.get("combat").is_some() {
                base.remove("tavern");
            }
            if raw.get("tavern").is_some() {
                base.remove("combat");
                base.remove("batch");
            }
            let mut merged = Value::Mapping(base);
            deep_merge(&mut merged, raw);
            let name = merged
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("scenario #{} has no `name:`", i + 1))?
                .to_string();
            let snake = name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
            if name.is_empty() || !snake {
                return Err(format!(
                    "scenario name {name:?} is not snake_case ([a-z0-9_]+)"
                ));
            }
            if !names.insert(name.clone()) {
                return Err(format!("two scenarios are named {name:?}"));
            }
            scenarios.push(Scenario::compile(name, merged));
        }
        Ok(ScenarioFile { header, scenarios })
    }
}

/// Merge `over` into `base`: mappings merge key by key, other values replace.
fn deep_merge(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Mapping(base), Value::Mapping(over)) => {
            for (key, value) in over {
                match base.get_mut(&key) {
                    Some(slot) => deep_merge(slot, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, over) => *base = over,
    }
}

impl Scenario {
    fn compile(name: String, value: Value) -> Scenario {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
        Scenario {
            description: text("description"),
            known_bug: text("known_bug"),
            body: compile_body(value),
            name,
        }
    }
}

fn compile_body(value: Value) -> Result<Body, String> {
    let raw: RawScenario = serde_yaml::from_value(value).map_err(|e| e.to_string())?;
    let seeds = match (raw.seed, &raw.seeds) {
        (Some(_), Some(_)) => return Err("`seed` and `seeds` are exclusive".to_string()),
        (Some(seed), None) => vec![seed],
        (None, Some(seeds)) => parse_seeds(seeds)?,
        (None, None) => vec![0],
    };
    let has_steps = raw.steps.is_some();
    let mut steps = compile_steps(raw.steps.as_deref().unwrap_or_default(), "")?;
    if let Some(expect) = raw.expect {
        steps.push(Step {
            desc: "expect".to_string(),
            kind: StepKind::Expect(expect),
        });
    }
    let setup = match (raw.tavern, raw.combat, raw.batch) {
        (Some(_), Some(_), _) => return Err("`tavern` and `combat` are exclusive".to_string()),
        (_, None, Some(_)) => return Err("`batch` needs `combat`".to_string()),
        (None, Some(combat), Some(batch)) => {
            if has_steps {
                return Err("a `batch` scenario has no `steps`, only `expect`".to_string());
            }
            if batch.n == 0 {
                return Err("`batch.n` must be at least 1".to_string());
            }
            let plan = compile_combat(&combat)?;
            standalone(&plan)?;
            Setup::Batch(plan, batch)
        }
        (None, Some(combat), None) => {
            combat_only(&steps)?;
            let plan = compile_combat(&combat)?;
            standalone(&plan)?;
            Setup::Combat(plan)
        }
        (tavern, None, None) => {
            let mut patch = match tavern {
                None | Some(Value::Null) => Mapping::new(),
                Some(Value::Mapping(m)) => m,
                Some(other) => {
                    return Err(format!("`tavern:` must be a mapping, got {}", show(&other)))
                }
            };
            let catalog = match patch.remove("catalog") {
                None => "full".to_string(),
                Some(Value::String(name)) => name,
                Some(other) => {
                    return Err(format!(
                        "`tavern.catalog` must be a catalog name, got {}",
                        show(&other)
                    ))
                }
            };
            cards::catalog_for(&catalog).map_err(|e| format!("tavern.catalog: {e}"))?;
            Setup::Tavern {
                catalog,
                patch: Value::Mapping(patch),
            }
        }
    };
    Ok(Body {
        seeds,
        setup,
        steps,
    })
}

/// `seeds: 1..50`, `1..=50`, a list or a single seed.
fn parse_seeds(value: &Value) -> Result<Vec<u64>, String> {
    let bad = || {
        format!(
            "`seeds` must be a range (\"1..50\", \"1..=50\"), a list or a seed, got {}",
            show(value)
        )
    };
    let seeds: Vec<u64> = match value {
        Value::Number(n) => vec![n.as_u64().ok_or_else(bad)?],
        Value::Sequence(list) => list
            .iter()
            .map(|v| v.as_u64().ok_or_else(bad))
            .collect::<Result<_, _>>()?,
        Value::String(s) => {
            let (lo, hi, inclusive) = match s.split_once("..=") {
                Some((lo, hi)) => (lo, hi, true),
                None => {
                    let (lo, hi) = s.split_once("..").ok_or_else(bad)?;
                    (lo, hi, false)
                }
            };
            let lo: u64 = lo.trim().parse().map_err(|_| bad())?;
            let hi: u64 = hi.trim().parse().map_err(|_| bad())?;
            if inclusive {
                (lo..=hi).collect()
            } else {
                (lo..hi).collect()
            }
        }
        _ => return Err(bad()),
    };
    if seeds.is_empty() {
        return Err(format!("`seeds` {} is empty", show(value)));
    }
    if seeds.len() > 10_000 {
        return Err(format!("`seeds` {} has more than 10000 seeds", show(value)));
    }
    Ok(seeds)
}

/// A standalone combat cannot copy from the Tavern.
fn standalone(plan: &CombatPlan) -> Result<(), String> {
    let from_tavern = matches!(plan.board_a, FromTavern::Tavern)
        || matches!(plan.board_b, FromTavern::Tavern)
        || matches!(plan.hand_a, FromTavern::Tavern)
        || matches!(plan.hand_b, FromTavern::Tavern)
        || matches!(plan.auras_a, FromTavern::Tavern)
        || matches!(plan.auras_b, FromTavern::Tavern);
    if from_tavern {
        Err("a `combat:` scenario has no Tavern to copy from (`tavern`)".to_string())
    } else {
        Ok(())
    }
}

/// A `combat:` scenario only simulates and checks.
fn combat_only(steps: &[Step]) -> Result<(), String> {
    for step in steps {
        match &step.kind {
            StepKind::Simulate(plan) => {
                standalone(plan).map_err(|e| format!("step `{}`: {e}", step.desc))?
            }
            StepKind::Expect(_) => {}
            StepKind::If {
                then, otherwise, ..
            } => {
                combat_only(then)?;
                combat_only(otherwise)?;
            }
            StepKind::Repeat { steps, .. } => combat_only(steps)?,
            _ => {
                return Err(format!(
                    "step `{}`: a `combat:` scenario only has simulate, expect, if and repeat \
                     steps",
                    step.desc
                ))
            }
        }
    }
    Ok(())
}

fn compile_steps(list: &[Value], prefix: &str) -> Result<Vec<Step>, String> {
    list.iter()
        .enumerate()
        .map(|(i, value)| {
            let desc = describe(value);
            let kind = step_kind(value, &format!("{prefix}{}.", i + 1))
                .map_err(|e| format!("step {prefix}{} `{desc}`: {e}", i + 1))?;
            Ok(Step { desc, kind })
        })
        .collect()
}

/// A compact description of a step.
fn describe(value: &Value) -> String {
    match value
        .as_mapping()
        .and_then(|m| m.keys().next())
        .and_then(Value::as_str)
    {
        Some(key @ ("expect" | "if" | "repeat")) => key.to_string(),
        _ => {
            let mut text = show(value);
            if text.len() > 100 {
                let mut end = 100;
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                text.truncate(end);
                text.push_str("...");
            }
            text
        }
    }
}

fn step_kind(value: &Value, prefix: &str) -> Result<StepKind, String> {
    let m = match value {
        Value::String(name) => {
            return match name.as_str() {
                "start_turn" => Ok(StepKind::StartTurn),
                "sync_all_auras" => Ok(StepKind::SyncAllAuras),
                "refresh" => Ok(StepKind::Action(Action::Refresh)),
                "upgrade_tavern" => Ok(StepKind::Action(Action::UpgradeTavern)),
                "toggle_freeze" => Ok(StepKind::Action(Action::ToggleFreeze)),
                "end_turn" => Ok(StepKind::Action(Action::EndTurn)),
                other => Err(unknown_step(other)),
            };
        }
        Value::Mapping(m) => m,
        other => {
            return Err(format!(
                "a step is a name or a mapping, got {}",
                show(other)
            ))
        }
    };
    if m.contains_key("if") {
        only_keys(m, &["if", "then", "else"])?;
        let branch = |key: &str| match m.get(key) {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(Value::Sequence(list)) => compile_steps(list, &format!("{prefix}{key}.")),
            Some(other) => Err(format!(
                "`{key}` must be a list of steps, got {}",
                show(other)
            )),
        };
        return Ok(StepKind::If {
            condition: m.get("if").cloned().unwrap_or_default(),
            then: branch("then")?,
            otherwise: branch("else")?,
        });
    }
    if m.contains_key("repeat") {
        only_keys(m, &["repeat", "steps"])?;
        let times = m
            .get("repeat")
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or("`repeat` must be a count")?;
        let steps = match m.get("steps") {
            Some(Value::Sequence(list)) => compile_steps(list, prefix)?,
            _ => return Err("`repeat` needs `steps: [...]`".to_string()),
        };
        return Ok(StepKind::Repeat { times, steps });
    }
    if m.len() != 1 {
        return Err(
            "a step is a single-key mapping (`buy: 0`), if/then/else or repeat/steps".to_string(),
        );
    }
    let (key, arg) = m.iter().next().expect("one key");
    let name = key
        .as_str()
        .ok_or_else(|| format!("unknown step {}", show(key)))?;
    Ok(match name {
        "buy" => StepKind::Action(Action::Buy(index(arg)?)),
        "sell" => StepKind::Action(Action::Sell(index(arg)?)),
        "choose_discover" => StepKind::Action(Action::ChooseDiscover(index(arg)?)),
        "play" => StepKind::Action(match arg.as_mapping() {
            Some(m) if m.contains_key("hand") => {
                only_keys(m, &["hand", "pos"])?;
                Action::Play {
                    hand: index(&m["hand"])?,
                    pos: optional_usize(m, "pos")?,
                }
            }
            _ => Action::Play {
                hand: index(arg)?,
                pos: None,
            },
        }),
        "reposition" => {
            let m = arg.as_mapping().ok_or("`reposition` takes {from, to}")?;
            only_keys(m, &["from", "to"])?;
            StepKind::Action(Action::Reposition {
                from: index(m.get("from").ok_or("`reposition` needs `from`")?)?,
                to: optional_usize(m, "to")?.ok_or("`reposition` needs `to`")?,
            })
        }
        "activate" => StepKind::Action(match arg.as_mapping() {
            Some(m) if m.contains_key("pos") => {
                only_keys(m, &["pos", "target"])?;
                Action::Activate {
                    pos: index(&m["pos"])?,
                    target: optional_usize(m, "target")?,
                }
            }
            _ => Action::Activate {
                pos: index(arg)?,
                target: None,
            },
        }),
        "add_to_hand" => StepKind::AddToHand(units::build_list(arg)?),
        "push_board" => StepKind::Push(Zone::Board, units::build_list(arg)?),
        "push_hand" => StepKind::Push(Zone::Hand, units::build_list(arg)?),
        "push_shop" => StepKind::Push(Zone::Shop, units::build_list(arg)?),
        "clear" => StepKind::Clear(match arg {
            Value::String(zone) => vec![Zone::parse(zone)?],
            Value::Sequence(zones) => zones
                .iter()
                .map(|z| Zone::parse(z.as_str().unwrap_or_default()))
                .collect::<Result<_, _>>()?,
            other => {
                return Err(format!(
                    "`clear` takes a zone or a list of zones, got {}",
                    show(other)
                ))
            }
        }),
        "set" => {
            if !arg.is_mapping() {
                return Err(format!(
                    "`set` takes a mapping of fields, got {}",
                    show(arg)
                ));
            }
            StepKind::Set(arg.clone())
        }
        "deal_hero_damage" => StepKind::DealHeroDamage(
            arg.as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .ok_or("`deal_hero_damage` takes an amount")?,
        ),
        "apply_global_unit_auras" => {
            let m = arg
                .as_mapping()
                .filter(|m| m.len() == 1)
                .ok_or("`apply_global_unit_auras` takes {<zone>: <index>}")?;
            let (zone, at) = m.iter().next().expect("one key");
            StepKind::ApplyGlobalUnitAuras(
                Zone::parse(zone.as_str().unwrap_or_default())?,
                index(at)?,
            )
        }
        "fight" => StepKind::Fight(Box::new(compile_fight(arg)?)),
        "simulate" => StepKind::Simulate(Box::new(compile_combat(arg)?)),
        "expect" => StepKind::Expect(arg.clone()),
        "expect_error" => StepKind::ExpectError(action(arg, prefix)?),
        "expect_legal" => StepKind::ExpectLegal(action(arg, prefix)?, true),
        "expect_illegal" => StepKind::ExpectLegal(action(arg, prefix)?, false),
        other => return Err(unknown_step(other)),
    })
}

fn unknown_step(name: &str) -> String {
    format!("unknown step `{name}` (steps: {STEP_NAMES})")
}

/// The Tavern action a step performs (`expect_error: buy: 0`).
fn action(value: &Value, prefix: &str) -> Result<Action, String> {
    match step_kind(value, prefix)? {
        StepKind::Action(action) => Ok(action),
        _ => Err(format!("{} is not a Tavern action", show(value))),
    }
}

fn index(value: &Value) -> Result<Index, String> {
    match value {
        Value::Number(n) => n
            .as_i64()
            .map(Index::At)
            .ok_or_else(|| format!("bad index {n}")),
        Value::String(_) | Value::Mapping(_) => Ok(Index::Find(value.clone())),
        other => Err(format!(
            "expected an index or a matcher, got {}",
            show(other)
        )),
    }
}

fn optional_usize(m: &Mapping, key: &str) -> Result<Option<usize>, String> {
    match m.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .map(Some)
            .ok_or_else(|| format!("`{key}` must be a position, got {}", show(v))),
    }
}

fn only_keys(m: &Mapping, keys: &[&str]) -> Result<(), String> {
    for key in m.keys() {
        if !key.as_str().is_some_and(|k| keys.contains(&k)) {
            return Err(format!(
                "unknown key {} (keys: {})",
                show(key),
                keys.join(", ")
            ));
        }
    }
    Ok(())
}

fn units_or_tavern(value: &Value, field: &str) -> Result<FromTavern<Vec<Unit>>, String> {
    match value.as_str() {
        Some("tavern") => Ok(FromTavern::Tavern),
        _ => units::build_list(value)
            .map(FromTavern::Given)
            .map_err(|e| format!("{field}: {e}")),
    }
}

fn auras(value: &Value, field: &str) -> Result<PlayerAuras, String> {
    let mut auras = PlayerAuras::default();
    if !value.is_null() {
        patch::apply(&mut auras, value).map_err(|e| format!("{field}: {e}"))?;
    }
    Ok(auras)
}

fn auras_or_tavern(value: &Value, field: &str) -> Result<FromTavern<PlayerAuras>, String> {
    match value.as_str() {
        Some("tavern") => Ok(FromTavern::Tavern),
        _ => auras(value, field).map(FromTavern::Given),
    }
}

fn compile_combat(value: &Value) -> Result<CombatPlan, String> {
    let raw: RawCombat =
        serde_yaml::from_value(value.clone()).map_err(|e| format!("combat: {e}"))?;
    Ok(CombatPlan {
        seed: raw.seed,
        board_a: units_or_tavern(&raw.board_a, "board_a")?,
        board_b: units_or_tavern(&raw.board_b, "board_b")?,
        hand_a: units_or_tavern(&raw.hand_a, "hand_a")?,
        hand_b: units_or_tavern(&raw.hand_b, "hand_b")?,
        hero_tier_a: raw.hero_tier_a,
        hero_tier_b: raw.hero_tier_b,
        auras_a: auras_or_tavern(&raw.auras_a, "auras_a")?,
        auras_b: auras_or_tavern(&raw.auras_b, "auras_b")?,
    })
}

fn compile_fight(value: &Value) -> Result<FightPlan, String> {
    let raw: RawFight = serde_yaml::from_value(value.clone()).map_err(|e| format!("fight: {e}"))?;
    Ok(FightPlan {
        seed: raw.seed,
        board: units::build_list(&raw.board).map_err(|e| format!("board: {e}"))?,
        tier: raw.tier,
        auras: auras(&raw.auras, "auras")?,
        hand: units::build_list(&raw.hand).map_err(|e| format!("hand: {e}"))?,
    })
}
