//! `match_replay scenario`: a scenario (`engine/docs/scenarios.md`) that reproduces a frame of a
//! replay (`engine/docs/replays.md` §4).
//!
//! The scenario rebuilds player P's Tavern at frame N exactly, replays P's next K recorded
//! actions and expects the state they lead to:
//!
//! - `tavern:` patches `TavernState::new()` with every field of P's state that differs from it.
//!   Units, wherever they are (the zones, the lobby's warbands, a Hero Power's or trinket's kept
//!   cards, ...), are unit specs (`"Alleycat +1/+1 taunt"`) plus, as a mapping, the fields the
//!   spec does not get right (`{card: ..., charges: 1}`): candidate specs are built with
//!   [`build_unit`] (exactly as the runner builds them) and diffed against the unit, and the
//!   candidate needing the fewest extra fields that builds the unit exactly wins. The hero and
//!   trinkets are named (`hero: {hero: <name>, power: {...}}`), with the fields that differ from
//!   a fresh one;
//! - `seed:` is the state of the RNG P draws from at frame N (the runner's `Rng::new(seed)`
//!   resumes it);
//! - a `take_from_pool` step takes the copies missing from the pool P draws from;
//! - the other steps are P's actions, and `expect:` checks Gold, Health, Armor, tier, every zone
//!   (unit specs as matchers) and whatever else the actions changed, the pool included.
//!
//! P's actions need not be consecutive frames. With a pool and RNG per player, the other
//! players' calls (their actions and `start_turn`s, combats without P) cannot touch P's Tavern,
//! pool or RNG, so the export passes over them (and checks that they did not); it stops at P's
//! own `start_turn`, a combat involving P or the end of the match. With one pool and RNG shared by
//! everyone, every call can move them, so it stops at the first frame that is not P's action.
//!
//! [`Export::check`] first rebuilds the setup exactly as the scenario runner does
//! ([`apply_patch`] on `TavernState::new()`, a fresh pool minus the `take_from_pool` copies,
//! `Rng::new(seed)`), replays the actions, and compares the complete Tavern state (`==`, but for
//! what only lives within a step: [`step_state`]), pool and RNG state with the replay's at every
//! one of P's frames; then it runs the scenario through the engine's scenario runner. An export
//! that fails either is an error, and is not written.

mod yaml;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::ExitCode;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_yaml::{Mapping, Value};

use seaglass::replay::{engine_version, pb, FrameKind, Timeline};
use seaglass::scenario::{apply_patch, build_unit, card_index, slug, Card, ScenarioFile};
use seaglass::{
    base_copies_for_tier, catalog_for, heroes, trinkets, CardId, CardPool, HeroPowerState,
    HeroState, Keyword, LobbyView, OpponentView, PlayerAuras, Rng, TavernAction, TavernState,
    TrinketState, Unit,
};
use seaglass_tools::replay_text::{frame_title, player_name};

use crate::{frame_arg, load_one, option};
use yaml::Layout;

const USAGE: &str = "\
usage: match_replay scenario <file> (--frame N | --turn T) [--steps K] [--player P] [-o out.yaml]

Writes a scenario (engine/docs/scenarios.md) that rebuilds player P's Tavern at frame N exactly,
replays P's next K recorded actions (default 1; 0 only rebuilds the state) and expects the state
they lead to. P is a player name or a 0-based index, by default whoever acts at frame N+1;
--turn T picks the frame before the first action of turn T. With a pool and RNG per player, the
other players' calls cannot touch P's Tavern and are passed over: the export stops early at P's
own start_turn, a combat involving P or the end of the match. With a shared pool and RNG, it
stops early at any frame that is not an action by P. The scenario is run before it is written:
the export fails if it does not pass.";

/// The options that take a value.
const VALUE_OPTIONS: [&str; 10] = [
    "--frame", "-f", "--turn", "-t", "--steps", "-k", "--player", "-p", "--output", "-o",
];

/// Every keyword, in the order the scenario view lists them.
const KEYWORDS: [Keyword; 7] = [
    Keyword::Taunt,
    Keyword::DivineShield,
    Keyword::Windfury,
    Keyword::Reborn,
    Keyword::Venomous,
    Keyword::Stealth,
    Keyword::Magnetic,
];

/// The fields `expect:` always checks (other fields only if the actions change them).
const ALWAYS_EXPECTED: [&str; 8] = [
    "gold",
    "health",
    "armor",
    "tavern_tier",
    "board",
    "hand",
    "shop",
    "discover_pending",
];

/// The width comment lines wrap at.
const COMMENT_WIDTH: usize = 96;

/// The longest scenario name: longer lists of actions end in `_and_<n>_more`.
const MAX_NAME: usize = 80;

pub fn run(args: &[String]) -> Result<ExitCode, String> {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return Ok(ExitCode::SUCCESS);
    }
    check_args(args)?;
    let timeline = load_one(args)?;
    let player = option(args, &["--player", "-p"])
        .map(|p| parse_player(&timeline, p))
        .transpose()?;
    let frame = pick_frame(&timeline, args, player)?;
    let steps = match option(args, &["--steps", "-k"]) {
        Some(k) => k
            .parse()
            .map_err(|_| format!("--steps needs a number, not {k:?}"))?,
        None => 1,
    };
    let export = export(&timeline, &args[0], frame, steps, player)?;
    export.check()?;
    if let Some(note) = &export.note {
        eprintln!("note: the scenario {note}");
    }
    match option(args, &["--output", "-o"]) {
        Some(path) => {
            std::fs::write(path, &export.yaml).map_err(|e| format!("cannot write {path}: {e}"))?;
            eprintln!(
                "wrote {path}: scenario {} ({}; it passes)",
                export.name,
                plural(export.actions, "action")
            );
        }
        None => print!("{}", export.yaml),
    }
    Ok(ExitCode::SUCCESS)
}

/// Reject unknown options, options without a value and stray arguments.
fn check_args(args: &[String]) -> Result<(), String> {
    if args.first().is_none_or(|a| a.starts_with('-')) {
        return Err(format!("missing the replay file\n\n{USAGE}"));
    }
    let mut i = 1;
    while i < args.len() {
        let arg = args[i].as_str();
        if VALUE_OPTIONS.contains(&arg) {
            if i + 1 >= args.len() {
                return Err(format!("{arg} needs a value\n\n{USAGE}"));
            }
            i += 2;
        } else if arg.starts_with('-') {
            return Err(format!("unknown option {arg:?}\n\n{USAGE}"));
        } else {
            return Err(format!("unexpected argument {arg:?}\n\n{USAGE}"));
        }
    }
    Ok(())
}

/// A player given by name (case-insensitive) or 0-based index.
fn parse_player(timeline: &Timeline, text: &str) -> Result<usize, String> {
    let players = timeline.num_players();
    if let Some(p) = (0..players).find(|&p| player_name(timeline, p).eq_ignore_ascii_case(text)) {
        return Ok(p);
    }
    match text.parse::<usize>() {
        Ok(p) if p < players => Ok(p),
        _ => {
            let names: Vec<String> = (0..players)
                .map(|p| format!("{} ({p})", player_name(timeline, p)))
                .collect();
            Err(format!(
                "no player {text:?} (players: {})",
                names.join(", ")
            ))
        }
    }
}

/// The frame to export: `--frame N`, or the frame before the first action (by `player`, if
/// given) of turn `--turn T`.
fn pick_frame(
    timeline: &Timeline,
    args: &[String],
    player: Option<usize>,
) -> Result<usize, String> {
    let by_turn = option(args, &["--frame", "-f"]).is_none();
    let frame = frame_arg(timeline, args)?
        .ok_or_else(|| format!("which frame? pass --frame N or --turn T\n\n{USAGE}"))?;
    if !by_turn {
        return Ok(frame);
    }
    (frame.max(1)..timeline.len())
        .find(|&j| acts(timeline, j, player))
        .map(|j| j - 1)
        .ok_or_else(|| {
            format!(
                "no action{} from turn {} on",
                by(timeline, player),
                timeline.frames[frame].turn
            )
        })
}

/// A scenario exported from a replay ([`export`]).
pub(crate) struct Export {
    /// The scenario's name.
    pub name: String,
    /// The scenario file.
    pub yaml: String,
    /// How many actions the scenario replays.
    pub actions: usize,
    /// Why the scenario replays fewer actions than asked for, if it does.
    pub note: Option<String>,
    /// What the scenario must reproduce exactly.
    expected: Expected,
}

impl Export {
    /// Check the export: rebuild its setup and replay its actions exactly as the scenario runner
    /// does, comparing every state, pool and RNG with the replay's; then run the scenario
    /// through the engine's scenario runner.
    pub(crate) fn check(&self) -> Result<(), String> {
        self.expected.reproduce().map_err(|e| {
            format!(
                "the exported scenario does not rebuild the replay exactly (a bug in \
                 `match_replay scenario`): {e}\n\n--- the scenario:\n{}",
                self.yaml
            )
        })?;
        run_scenarios(&self.yaml).map_err(|e| {
            format!(
                "the exported scenario does not pass: {e}\n\n--- the scenario:\n{}",
                self.yaml
            )
        })
    }
}

/// Parse a scenario file and run every scenario in it.
fn run_scenarios(text: &str) -> Result<(), String> {
    let file = ScenarioFile::parse(text)?;
    for scenario in &file.scenarios {
        scenario
            .run()
            .map_err(|failure| format!("scenario {}: {failure}", scenario.name))?;
    }
    Ok(())
}

/// What an exported scenario must rebuild: its setup (as the runner builds it from `tavern:`,
/// `take_from_pool` and `seed:`) and the frames its actions lead to.
struct Expected {
    catalog: String,
    /// The `tavern:` patch, without `catalog`.
    patch: Value,
    /// The cards `take_from_pool` takes, one per copy.
    taken: Vec<CardId>,
    seed: u64,
    /// The exported frame, then the frame after each action.
    checkpoints: Vec<Checkpoint>,
}

/// A frame an exported scenario must reproduce exactly.
struct Checkpoint {
    frame: usize,
    /// The action that leads to the frame (none for the exported frame itself).
    action: Option<TavernAction>,
    state: TavernState,
    pool: Vec<u32>,
    rng: u64,
}

impl Expected {
    /// Rebuild the setup like the scenario runner (`TavernState::new()` patched, a fresh pool,
    /// `Rng::new(seed)`), replay the actions, and compare every checkpoint.
    fn reproduce(&self) -> Result<(), String> {
        let mut state = TavernState::new();
        apply_patch(&mut state, &self.patch).map_err(|e| format!("tavern: {e}"))?;
        let catalog = catalog_for(&self.catalog)?;
        let mut pool = CardPool::new(catalog.clone());
        for &card in &self.taken {
            pool.take_copy(card);
        }
        let mut rng = Rng::new(self.seed);
        for checkpoint in &self.checkpoints {
            let at = match checkpoint.action {
                Some(action) => {
                    state.step(action, &mut pool, &mut rng).map_err(|e| {
                        format!(
                            "the engine rejected the action of frame {} ({action:?}): {e}",
                            checkpoint.frame
                        )
                    })?;
                    format!("after the action of frame {}", checkpoint.frame)
                }
                None => format!("at frame {}", checkpoint.frame),
            };
            if step_state(&state) != step_state(&checkpoint.state) {
                let diff = difference(&to_value(&state)?, &to_value(&checkpoint.state)?, "")
                    .unwrap_or_else(|| "they differ".to_string());
                return Err(format!("the Tavern {at} is not the replay's: {diff}"));
            }
            let remaining = pool.remaining();
            if remaining.len() != checkpoint.pool.len() {
                return Err(format!(
                    "the pool {at} has {} cards, not the replay's {}",
                    remaining.len(),
                    checkpoint.pool.len()
                ));
            }
            if let Some(i) = (0..remaining.len()).find(|&i| remaining[i] != checkpoint.pool[i]) {
                return Err(format!(
                    "the pool {at} is not the replay's: {} copies of {}, not {}",
                    remaining[i], catalog[i].name, checkpoint.pool[i]
                ));
            }
            if rng.state() != checkpoint.rng {
                return Err(format!(
                    "the RNG {at} is in state {}, not the replay's {}",
                    rng.state(),
                    checkpoint.rng
                ));
            }
        }
        Ok(())
    }
}

/// `state` without what only lives within a step (the fields serde skips, which a scenario
/// cannot set): the minions sold during the last step, which `TavernState::step` forgets as the
/// next one starts, and the Triples deferred during one.
fn step_state(state: &TavernState) -> TavernState {
    TavernState {
        sold_this_step: Vec::new(),
        triples_deferred: false,
        ..state.clone()
    }
}

/// The first difference between two values, as `path: got X, want Y`.
fn difference(got: &Value, want: &Value, path: &str) -> Option<String> {
    let join = |name: String| {
        if path.is_empty() {
            name
        } else {
            format!("{path}.{name}")
        }
    };
    match (got, want) {
        (Value::Mapping(g), Value::Mapping(w)) => {
            for (key, wanted) in w {
                let key_path = join(yaml::flow(key));
                match g.get(key) {
                    Some(value) => {
                        if let Some(diff) = difference(value, wanted, &key_path) {
                            return Some(diff);
                        }
                    }
                    None => return Some(format!("{key_path}: missing")),
                }
            }
            g.keys()
                .find(|key| !w.contains_key(*key))
                .map(|key| format!("{}: unexpected", join(yaml::flow(key))))
        }
        (Value::Sequence(g), Value::Sequence(w)) if g.len() == w.len() => g
            .iter()
            .zip(w)
            .enumerate()
            .find_map(|(i, (value, wanted))| difference(value, wanted, &format!("{path}[{i}]"))),
        _ if got == want => None,
        _ => Some(format!(
            "{path}: got {}, want {}",
            yaml::flow(got),
            yaml::flow(want)
        )),
    }
}

/// One of the recorded actions a scenario replays.
struct Act {
    /// The frame the action produced.
    frame: usize,
    action: TavernAction,
    /// The card the action used.
    card: Option<CardId>,
}

/// What an export replays: player `p`'s actions after frame `frame`.
struct Run {
    frame: usize,
    p: usize,
    /// Whether every player has their own pool and RNG.
    per_player: bool,
    /// The RNG and the pool `p` draws from (indices into `Frame::rng_states` and
    /// `Frame::pool_remaining`).
    rng_slot: usize,
    pool_slot: usize,
    actions: Vec<Act>,
    /// The other players' calls between `frame` and the last action, which the run passes over
    /// (per-player pools only).
    passed: Vec<usize>,
    /// Why the run replays fewer actions than asked for, if it does.
    note: Option<String>,
}

impl Run {
    /// Player `p`'s next `steps` actions after frame `frame`. With a pool and RNG per player, the
    /// other players' calls (which cannot touch `p`'s Tavern, pool or RNG) are passed over, and
    /// the run stops at the first frame involving `p` that is not their action; with a shared
    /// pool and RNG, it stops at the first frame that is not `p`'s action.
    fn new(timeline: &Timeline, frame: usize, steps: usize, p: usize) -> Result<Run, String> {
        let start = &timeline.frames[frame];
        // One RNG and pool per player, or one shared by everyone.
        let slot = |n: usize| if n == 1 { 0 } else { p };
        let mut run = Run {
            frame,
            p,
            per_player: timeline.header.pool() == pb::PoolMode::PerPlayer,
            rng_slot: slot(start.rng_states.len()),
            pool_slot: slot(start.pool_remaining.len()),
            actions: Vec::new(),
            passed: Vec::new(),
            note: None,
        };
        let mut passing = Vec::new();
        let mut i = frame + 1;
        while run.actions.len() < steps {
            let Some(next) = timeline.frames.get(i) else {
                break;
            };
            match &next.kind {
                FrameKind::Act {
                    player,
                    action,
                    card_id,
                    ..
                } if *player == p => {
                    run.passed.append(&mut passing);
                    run.actions.push(Act {
                        frame: i,
                        action: *action,
                        card: *card_id,
                    });
                }
                kind if run.per_player && !involves(kind, p) && run.untouched(timeline, i) => {
                    passing.push(i)
                }
                _ => break,
            }
            i += 1;
        }
        if run.actions.len() == steps {
            return Ok(run);
        }
        // The run stopped early, at frame `i` (`timeline.len()`: the replay ends).
        if run.actions.is_empty() {
            let who = player_name(timeline, p);
            let reason = if i < timeline.len() {
                run.stop_reason(timeline, i)
            } else if frame + 1 < timeline.len() {
                format!(
                    "no action by {who} follows frame {frame}: the replay ends at frame {}{}",
                    timeline.len() - 1,
                    stopped(timeline)
                )
            } else {
                format!(
                    "frame {frame} is the last frame: no action by {who} follows it{}",
                    stopped(timeline)
                )
            };
            return Err(no_action(timeline, i, reason, Some(p), steps));
        }
        run.note = Some(format!(
            "replays {} of the {steps} actions asked for: {}",
            run.actions.len(),
            run.stop_reason(timeline, i)
        ));
        Ok(run)
    }

    /// Whether frame `i` left the Tavern, pool and RNG of the run's player as they were.
    fn untouched(&self, timeline: &Timeline, i: usize) -> bool {
        let (before, after) = (&timeline.frames[i - 1], &timeline.frames[i]);
        before.players[self.p] == after.players[self.p]
            && before.pool_remaining[self.pool_slot] == after.pool_remaining[self.pool_slot]
            && before.rng_states[self.rng_slot] == after.rng_states[self.rng_slot]
    }

    /// Why the run stops at frame `at` (`timeline.len()`: the replay ends there).
    fn stop_reason(&self, timeline: &Timeline, at: usize) -> String {
        if at >= timeline.len() {
            return format!(
                "the replay ends at frame {}{}",
                timeline.len() - 1,
                stopped(timeline)
            );
        }
        let who = player_name(timeline, self.p);
        let title = frame_title(timeline, at);
        if involves(&timeline.frames[at].kind, self.p) {
            format!("frame {at} is not an action by {who} ({title})")
        } else if self.per_player {
            format!("frame {at} ({title}) changes {who}'s Tavern, pool or RNG")
        } else {
            format!(
                "frame {at} is another player's call ({title}), and all players draw from one \
                 pool and RNG"
            )
        }
    }
}

/// Export player `player`'s Tavern at frame `frame` of `timeline` (read from `source`) and their
/// next `steps` actions as a scenario. `player` defaults to whoever acts at frame `frame + 1`.
pub(crate) fn export(
    timeline: &Timeline,
    source: &str,
    frame: usize,
    steps: usize,
    player: Option<usize>,
) -> Result<Export, String> {
    if frame >= timeline.len() {
        return Err(format!(
            "frame {frame} does not exist (the replay has frames 0..={})",
            timeline.len() - 1
        ));
    }
    let next = frame + 1;
    let p = match player {
        Some(p) if p < timeline.num_players() => p,
        Some(p) => return Err(format!("player {p} does not exist")),
        None => match actor(timeline, next) {
            Some(p) => p,
            None => {
                let reason = if next < timeline.len() {
                    format!(
                        "frame {next} is not an action ({})",
                        frame_title(timeline, next)
                    )
                } else {
                    format!(
                        "frame {frame} is the last frame: no action follows it{}",
                        stopped(timeline)
                    )
                };
                return Err(no_action(timeline, next, reason, None, steps));
            }
        },
    };
    let run = Run::new(timeline, frame, steps, p)?;
    let who = player_name(timeline, p);
    let actions = &run.actions;
    let (rng_slot, pool_slot) = (run.rng_slot, run.pool_slot);

    let start = &timeline.frames[frame];
    let end = actions.last().map_or(frame, |a| a.frame);
    let state = &start.players[p];
    let after = &timeline.frames[end].players[p];
    let seed = start.rng_states[rng_slot];
    let pool = PoolCards::new(timeline)?;
    let missing = pool.missing(&start.pool_remaining[pool_slot])?;
    let catalog = &timeline.header.catalog;
    let patch = tavern_patch(state)?;
    let checkpoint = |i: usize, action: Option<TavernAction>| {
        let at = &timeline.frames[i];
        Checkpoint {
            frame: i,
            action,
            state: at.players[p].clone(),
            pool: at.pool_remaining[pool_slot].clone(),
            rng: at.rng_states[rng_slot],
        }
    };
    let expected = Expected {
        catalog: catalog.clone(),
        patch: patch.clone(),
        taken: missing.iter().map(|card| card.card_id).collect(),
        seed,
        checkpoints: std::iter::once(checkpoint(frame, None))
            .chain(actions.iter().map(|a| checkpoint(a.frame, Some(a.action))))
            .collect(),
    };

    let described: Vec<(String, String)> =
        actions.iter().map(|a| describe(a.action, a.card)).collect();
    let file = file_name(source);
    let words: Vec<&str> = described.iter().map(|(_, word)| word.as_str()).collect();
    let name = scenario_name(&stem(&file), frame, &who, &words);
    let description = if actions.is_empty() {
        format!(
            "{who}'s Tavern at frame {frame} (turn {}) of {file}.",
            state.turn
        )
    } else {
        let phrases: Vec<&str> = described
            .iter()
            .map(|(phrase, _)| phrase.as_str())
            .collect();
        format!(
            "{who} at frame {frame} (turn {}) of {file}: {}.",
            state.turn,
            phrases.join(", ")
        )
    };

    let mut layout = Layout {
        block: vec![
            "scenarios",
            "tavern",
            "steps",
            "expect",
            "board",
            "hand",
            "shop",
            "discover_pending",
            "discover_queue",
        ],
        block_items: vec!["scenarios", "steps"],
        ..Layout::default()
    };
    layout.trailing.insert(
        "scenarios.0.seed".to_string(),
        if run.per_player || timeline.num_players() == 1 {
            format!("{who}'s RNG at frame {frame}")
        } else {
            format!("the shared RNG at frame {frame}")
        },
    );

    let mut tavern = Mapping::new();
    if catalog != "full" {
        tavern.insert("catalog".into(), catalog.as_str().into());
    }
    if let Value::Mapping(fields) = patch {
        tavern.extend(fields);
    }
    let mut scenario = Mapping::new();
    scenario.insert("name".into(), name.as_str().into());
    scenario.insert("description".into(), description.into());
    scenario.insert("seed".into(), seed.into());
    scenario.insert("tavern".into(), Value::Mapping(tavern));
    let mut steps_list = Vec::new();
    if !missing.is_empty() {
        layout.leading.insert(
            "scenarios.0.steps.0".to_string(),
            vec![format!(
                "the {} missing from the pool at frame {frame}",
                plural(missing.len(), "copy")
            )],
        );
        let names = missing
            .iter()
            .map(|card| card.name.as_str().into())
            .collect();
        steps_list.push(single("take_from_pool", Value::Sequence(names)));
    }
    let mut before = state;
    for act in actions {
        let step = action_step(before, act.action);
        // Each step's frame (and card) as a comment on it.
        let path = match &step {
            Value::Mapping(m) => format!(
                "scenarios.0.steps.{}.{}",
                steps_list.len(),
                m.keys().next().and_then(Value::as_str).unwrap_or_default()
            ),
            _ => format!("scenarios.0.steps.{}", steps_list.len()),
        };
        let comment = match act.card {
            Some(card) => format!("frame {}: {}", act.frame, card_name(card)),
            None => format!("frame {}", act.frame),
        };
        layout.trailing.insert(path, comment);
        steps_list.push(step);
        before = &timeline.frames[act.frame].players[p];
    }
    if !steps_list.is_empty() {
        scenario.insert("steps".into(), Value::Sequence(steps_list));
    }
    let end_pool = &timeline.frames[end].pool_remaining[pool_slot];
    let pool_changes = pool.changes(&start.pool_remaining[pool_slot], end_pool);
    scenario.insert("expect".into(), expectation(state, after, pool_changes)?);
    layout.leading.insert(
        "scenarios.0.expect".to_string(),
        vec![if actions.is_empty() {
            format!("{who}'s state at frame {frame}")
        } else {
            format!("{who}'s state after them (frame {end})")
        }],
    );
    layout.header = header(timeline, source, &run, missing.len());

    let mut doc = Mapping::new();
    doc.insert(
        "topic".into(),
        format!("replay {file} frame {frame} ({who})").into(),
    );
    doc.insert(
        "scenarios".into(),
        Value::Sequence(vec![Value::Mapping(scenario)]),
    );
    let yaml = yaml::render(&Value::Mapping(doc), &layout)?;
    Ok(Export {
        name,
        yaml,
        actions: actions.len(),
        note: run.note.clone(),
        expected,
    })
}

/// The player who acts at frame `i`, if it is an action.
fn actor(timeline: &Timeline, i: usize) -> Option<usize> {
    match timeline.frames.get(i).map(|f| &f.kind) {
        Some(FrameKind::Act { player, .. }) => Some(*player),
        _ => None,
    }
}

/// Whether frame `i` is an action (by `player`, if given).
fn acts(timeline: &Timeline, i: usize, player: Option<usize>) -> bool {
    actor(timeline, i).is_some_and(|actor| player.is_none_or(|p| p == actor))
}

/// Whether a call involves player `p`: their own action or `start_turn`, a combat they fight
/// in, a lobby call (it can reach every player's Tavern), or the start or end of the match.
fn involves(kind: &FrameKind, p: usize) -> bool {
    match kind {
        FrameKind::Act { player, .. } | FrameKind::StartTurn { player, .. } => *player == p,
        FrameKind::Combat { a, b, .. } => *a == p || *b == p,
        FrameKind::Start
        | FrameKind::SetPairs { .. }
        | FrameKind::DeliverGifts
        | FrameKind::RecordDeaths { .. }
        | FrameKind::End { .. } => true,
    }
}

/// `" by <player>"`, or nothing.
fn by(timeline: &Timeline, player: Option<usize>) -> String {
    player.map_or_else(String::new, |p| format!(" by {}", player_name(timeline, p)))
}

/// Why the re-simulation stopped early, if it did (`" (...)"`).
fn stopped(timeline: &Timeline) -> String {
    timeline.error.as_ref().map_or_else(String::new, |e| {
        format!(
            " (the re-simulation stopped at record {}: {})",
            e.record, e.message
        )
    })
}

/// The error for an export with no action to replay: `reason` (why it stops at frame `at`), and
/// where the next action (by `player`, if given) is.
fn no_action(
    timeline: &Timeline,
    at: usize,
    reason: String,
    player: Option<usize>,
    steps: usize,
) -> String {
    let by = by(timeline, player);
    let mut message = reason;
    if at < timeline.len() {
        match (at + 1..timeline.len()).find(|&j| acts(timeline, j, player)) {
            Some(j) => message.push_str(&format!(
                "; the next action{by} is frame {j} ({}): use --frame {}",
                frame_title(timeline, j),
                j - 1
            )),
            None => message.push_str(&format!("; no later frame is an action{by}")),
        }
    }
    if steps == 0 && player.is_none() {
        message.push_str(" (with --steps 0, --player P exports P's state at any frame)");
    }
    message
}

/// `frame 3`, `frames 3 and 5`, `frames 3, 5 and 7`, ..., or `12 frames from 3 to 40`.
fn frame_list(frames: &[usize]) -> String {
    match frames {
        [] => String::new(),
        [one] => format!("frame {one}"),
        [rest @ .., last] if frames.len() <= 6 => {
            let rest: Vec<String> = rest.iter().map(usize::to_string).collect();
            format!("frames {} and {last}", rest.join(", "))
        }
        [first, .., last] => format!("{} frames from {first} to {last}", frames.len()),
    }
}

/// A card of a replay's pools.
struct PoolCard {
    name: String,
    card_id: CardId,
    /// Its copies in a fresh pool.
    fresh: u32,
}

/// The cards of a replay's pools, in pool order.
struct PoolCards(Vec<PoolCard>);

impl PoolCards {
    fn new(timeline: &Timeline) -> Result<Self, String> {
        let catalog = catalog_for(&timeline.header.catalog)?;
        let ids: Vec<CardId> = catalog.iter().map(|tpl| tpl.card_id).collect();
        if ids != timeline.card_ids {
            return Err(format!(
                "the {:?} catalog of this build is not the pool the replay was re-simulated with",
                timeline.header.catalog
            ));
        }
        let cards = catalog
            .iter()
            .map(|tpl| {
                let named = card_index()
                    .resolve(&tpl.name)
                    .is_ok_and(|card| card.card_id == tpl.card_id);
                if !named {
                    return Err(format!(
                        "pool card {:?} (#{}) cannot be named in a scenario",
                        tpl.name, tpl.card_id
                    ));
                }
                Ok(PoolCard {
                    name: tpl.name.clone(),
                    card_id: tpl.card_id,
                    fresh: base_copies_for_tier(tpl.tavern_tier),
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(PoolCards(cards))
    }

    /// One card for each copy missing from a pool with `remaining` copies, in pool order.
    fn missing(&self, remaining: &[u32]) -> Result<Vec<&PoolCard>, String> {
        if remaining.len() != self.0.len() {
            return Err(format!(
                "the replay's pool has {} cards, not the catalog's {}",
                remaining.len(),
                self.0.len()
            ));
        }
        let mut missing = Vec::new();
        for (card, &left) in self.0.iter().zip(remaining) {
            if left > card.fresh {
                return Err(format!(
                    "the pool holds {left} copies of {}, more than a fresh pool's {}: a scenario \
                     cannot set that up",
                    card.name, card.fresh
                ));
            }
            missing.extend(std::iter::repeat_n(card, (card.fresh - left) as usize));
        }
        Ok(missing)
    }

    /// `{card: copies}` for each card with different copies in `before` and `after`.
    fn changes(&self, before: &[u32], after: &[u32]) -> Mapping {
        let mut changes = Mapping::new();
        for ((card, &b), &a) in self.0.iter().zip(before).zip(after) {
            if a != b {
                changes.insert(card.name.as_str().into(), a.into());
            }
        }
        changes
    }
}

/// The `tavern:` patch that turns `TavernState::new()` into `state`.
///
/// It is written field by field, the way the patch reads it (`engine/src/scenario/patch.rs`):
/// [`struct_patch`] walks the serde form of the state, so every unit anywhere in it (the zones,
/// `lobby.opponents[].last_warband`, a Hero Power's or a trinket's `stored`, a unit's
/// `stitched_stored`, ...) is a unit spec wherever the patch takes one, and the player's auras,
/// hero and trinkets use the patch's own forms ([`auras_patch`]).
fn tavern_patch(state: &TavernState) -> Result<Value, String> {
    let fresh = to_value(&TavernState::new())?;
    let now = to_value(state)?;
    struct_patch(fields(&fresh)?, fields(&now)?).map(Value::Mapping)
}

/// Fields that hold a list of units, wherever they are: the patch takes them as unit specs
/// (`UNIT_LISTS` in `engine/src/scenario/patch.rs`).
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

/// Fields that hold a list of lists of units: the patch takes each list as unit specs
/// (`UNIT_LIST_LISTS` in `engine/src/scenario/patch.rs`).
const UNIT_LIST_LISTS: [&str; 2] = ["discover_queue", "incoming"];

/// The fields of `value` (a serialized struct) that differ from `base` (the same struct before
/// the patch), each as the patch takes it ([`field_patch`]). A field serde left out of `value`
/// (it has its default) that `base` has is set back to its default ([`reset`]).
fn struct_patch(base: &Mapping, value: &Mapping) -> Result<Mapping, String> {
    let filled = patched_base(base, value)?;
    let mut patch = Mapping::new();
    for (key, field) in value {
        let old = filled.get(key);
        if old == Some(field) {
            continue;
        }
        let name = key.as_str().unwrap_or_default();
        patch.insert(key.clone(), field_patch(old, field, name)?);
    }
    // (The fields the patch fills in have their default already.)
    for (key, old) in base {
        if value.contains_key(key) {
            continue;
        }
        let default = reset(old).ok_or_else(|| {
            format!(
                "cannot write a patch that resets `{}` (now {}) to its default",
                yaml::flow(key),
                yaml::flow(old)
            )
        })?;
        if default != *old {
            patch.insert(key.clone(), default);
        }
    }
    Ok(patch)
}

/// `base` with the fields the patch fills in before patching a struct of its kind (fields serde
/// leaves out at their default: a Tavern's `lobby`, `waiting_for_hand`, ..., a Hero Power's or a
/// trinket's `stored`, a hero's `second_power`), as `merge_fields` in
/// `engine/src/scenario/patch.rs` does: the patch merges into those.
fn patched_base(base: &Mapping, value: &Mapping) -> Result<Mapping, String> {
    let mut base = base.clone();
    let has = |m: &Mapping, key: &str| m.contains_key(key);
    let fill = |base: &mut Mapping, key: &str, default: Value| {
        if !base.contains_key(key) {
            base.insert(key.into(), default);
        }
    };
    let empty = || Value::Sequence(Vec::new());
    if has(value, "discover_pending") && has(value, "tavern_tier") {
        for key in [
            "waiting_for_hand",
            "lobby_gifts",
            "trinket_options",
            "trinket_offer_queue",
        ] {
            fill(&mut base, key, empty());
        }
        for key in [
            "trinkets_enabled",
            "dark_gifts_enabled",
            "dark_discovery_used_this_turn",
        ] {
            fill(&mut base, key, false.into());
        }
        fill(&mut base, "dark_discoveries_used", 0.into());
        fill(&mut base, "lobby", to_value(&LobbyView::default())?);
    }
    // A Hero Power.
    if has(value, "uses_left") && has(value, "armed") {
        fill(&mut base, "stored", empty());
    }
    // A trinket.
    if has(value, "stacks") && has(value, "armed") && !has(value, "uses_left") {
        fill(&mut base, "stored", empty());
        fill(&mut base, "tribe", "none".into());
    }
    // A hero: a second Hero Power merges into a fresh one.
    if has(value, "hero_id") && has(value, "power") {
        let fresh_second = value.get("second_power").is_some_and(Value::is_mapping)
            && base.get("second_power").is_none_or(Value::is_null);
        if fresh_second {
            base.insert("second_power".into(), to_value(&HeroPowerState::default())?);
        } else {
            fill(&mut base, "second_power", Value::Null);
        }
    }
    Ok(base)
}

/// The default of a field serde leaves out at its default, from its current value: `false`,
/// `0`, an empty list, or `null` for a struct (an `Option`). `None` if it cannot tell.
fn reset(old: &Value) -> Option<Value> {
    match old {
        Value::Bool(_) => Some(false.into()),
        Value::Number(_) => Some(0.into()),
        Value::Sequence(_) => Some(Value::Sequence(Vec::new())),
        Value::Mapping(_) | Value::Null => Some(Value::Null),
        _ => None,
    }
}

/// The patch of the field `name`, `old` before (`None`: the struct has no such field yet) and
/// `value` after: unit specs for units ([`UNIT_LISTS`], [`UNIT_LIST_LISTS`]), a patch of each
/// lobby opponent, [`auras_patch`] for player auras, the fields that differ for a struct, the
/// entries that differ for a map; anything else, and anything the patch replaces or inserts
/// as a whole (a field it has no value for), as it is.
fn field_patch(old: Option<&Value>, value: &Value, name: &str) -> Result<Value, String> {
    if let Some(old) = old {
        if UNIT_LISTS.contains(&name) {
            return match value {
                Value::Null => Ok(Value::Null),
                units => unit_specs(&from_value::<Vec<Unit>>(units)?),
            };
        }
        if UNIT_LIST_LISTS.contains(&name) && value.is_sequence() {
            let lists: Vec<Vec<Unit>> = from_value(value)?;
            return lists
                .iter()
                .map(|units| unit_specs(units))
                .collect::<Result<_, _>>()
                .map(Value::Sequence);
        }
        if name == "opponents" {
            // Each opponent is patched into a fresh `OpponentView`.
            let fresh = to_value(&OpponentView::default())?;
            let items = value.as_sequence().into_iter().flatten();
            return items
                .map(|item| {
                    Ok(Value::Mapping(struct_patch(
                        fields(&fresh)?,
                        fields(item)?,
                    )?))
                })
                .collect::<Result<_, String>>()
                .map(Value::Sequence);
        }
        if let (Value::Mapping(before), Value::Mapping(after)) = (old, value) {
            if is_auras(after) {
                return auras_patch(before, after).map(Value::Mapping);
            }
            if is_struct(before) && is_struct(after) {
                return struct_patch(before, after).map(Value::Mapping);
            }
            return Ok(Value::Mapping(map_patch(before, after)));
        }
    }
    Ok(untag(value.clone()))
}

/// Whether a serialized mapping is a struct (its keys are field names), not a map.
fn is_struct(m: &Mapping) -> bool {
    m.keys().all(Value::is_string)
}

/// Whether a serialized mapping is a `PlayerAuras` (as the patch tells).
fn is_auras(m: &Mapping) -> bool {
    m.contains_key("card_counters") && m.contains_key("effects")
}

/// The patch of a map (`{<key>: value}`): the entries that differ, and `null` for the ones it
/// lost.
fn map_patch(before: &Mapping, after: &Mapping) -> Mapping {
    let mut patch: Mapping = after
        .iter()
        .filter(|(key, value)| before.get(*key) != Some(*value))
        .map(|(key, value)| (key.clone(), untag(value.clone())))
        .collect();
    for key in before.keys().filter(|key| !after.contains_key(*key)) {
        patch.insert(key.clone(), Value::Null);
    }
    patch
}

/// The `auras:` patch that turns the player auras `before` into `after` (both serialized): card
/// counters as `counters: {<card>: [a, b]}`, effects by card name, the hero as `hero: <name>`
/// ([`hero_patch`]), trinkets as `trinkets: [<name>, ...]` ([`trinkets_patch`]), and the other
/// fields as [`struct_patch`] writes them.
fn auras_patch(before: &Mapping, after: &Mapping) -> Result<Mapping, String> {
    let old: PlayerAuras = from_value(&Value::Mapping(before.clone()))?;
    let new: PlayerAuras = from_value(&Value::Mapping(after.clone()))?;
    let special = ["card_counters", "effects", "hero", "trinkets"];
    let rest = |m: &Mapping| -> Mapping {
        m.iter()
            .filter(|(key, _)| !special.contains(&key.as_str().unwrap_or_default()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    };
    let mut patch = struct_patch(&rest(before), &rest(after))?;
    if new.card_counters != old.card_counters {
        let mut counters: Mapping = new
            .card_counters
            .iter()
            .filter(|(id, pair)| old.card_counters.get(id) != Some(pair))
            .map(|(&id, &pair)| (card_key(id), pair_value(pair)))
            .collect();
        for &id in old.card_counters.keys() {
            if !new.card_counters.contains_key(&id) {
                counters.insert(card_key(id), Value::Null);
            }
        }
        patch.insert("counters".into(), Value::Mapping(counters));
    }
    if new.effects != old.effects {
        let effects = new
            .effects
            .iter()
            .map(|effect| {
                let mut m = Mapping::new();
                m.insert("card".into(), card_key(effect.card_id));
                m.insert("stacks".into(), effect.stacks.into());
                m.insert("duration".into(), untag(to_value(&effect.duration)?));
                Ok(Value::Mapping(m))
            })
            .collect::<Result<_, String>>()?;
        patch.insert("effects".into(), Value::Sequence(effects));
    }
    if new.hero != old.hero {
        patch.insert("hero".into(), hero_patch(new.hero.as_ref())?);
    }
    if new.trinkets != old.trinkets {
        patch.insert("trinkets".into(), trinkets_patch(&new.trinkets)?);
    }
    Ok(patch)
}

/// The auras' `hero:` patch for `hero`: its name (a fresh hero, `heroes::new_state`), with the
/// fields that differ from a fresh one as `{hero: <name>, <field>: ...}`; `null` for none.
fn hero_patch(hero: Option<&HeroState>) -> Result<Value, String> {
    let Some(hero) = hero else {
        return Ok(Value::Null);
    };
    let data = heroes::data(hero.hero_id)
        .ok_or_else(|| format!("hero #{} is not in the hero table", hero.hero_id))?;
    if heroes::by_name(data.name).map(|h| h.id) != Some(hero.hero_id) {
        return Err(format!(
            "hero {:?} (#{}) cannot be named in a scenario",
            data.name, hero.hero_id
        ));
    }
    let fresh = to_value(&heroes::new_state(data))?;
    let diff = struct_patch(fields(&fresh)?, fields(&to_value(hero)?)?)?;
    Ok(named_patch("hero", data.name, diff))
}

/// The auras' `trinkets:` patch for `trinkets`: each by name (a fresh trinket,
/// `trinkets::new_state`), with the fields that differ from a fresh one as
/// `{trinket: <name>, <field>: ...}`.
fn trinkets_patch(list: &[TrinketState]) -> Result<Value, String> {
    list.iter()
        .map(|trinket| {
            let data = trinkets::data(trinket.card_id).ok_or_else(|| {
                format!("trinket #{} is not in the trinket table", trinket.card_id)
            })?;
            let name = trinkets::display_name(data);
            if trinkets::by_name(&name).map(|t| t.id) != Some(trinket.card_id) {
                return Err(format!(
                    "trinket {name:?} (#{}) cannot be named in a scenario",
                    trinket.card_id
                ));
            }
            let fresh = to_value(&trinkets::new_state(trinket.card_id))?;
            let diff = struct_patch(fields(&fresh)?, fields(&to_value(trinket)?)?)?;
            Ok(named_patch("trinket", &name, diff))
        })
        .collect::<Result<_, String>>()
        .map(Value::Sequence)
}

/// `name`, or `{<key>: name, <field>: ...}` with the fields of `diff` if it has any.
fn named_patch(key: &str, name: &str, diff: Mapping) -> Value {
    if diff.is_empty() {
        return name.into();
    }
    let mut m = Mapping::new();
    m.insert(key.into(), name.into());
    m.extend(diff);
    Value::Mapping(m)
}

/// What `expect:` checks after the actions took `before` to `after`: Gold, Health, Armor, tier,
/// the zones, every other field that changed, and the pool `changes`.
fn expectation(
    before: &TavernState,
    after: &TavernState,
    pool_changes: Mapping,
) -> Result<Value, String> {
    let (old, new) = (to_value(before)?, to_value(after)?);
    let mut expect = Mapping::new();
    for field in ["gold", "health", "armor", "tavern_tier"] {
        expect.insert(field.into(), new.get(field).cloned().unwrap_or_default());
    }
    for (key, value) in fields(&new)? {
        let field = key.as_str().unwrap_or_default();
        if ALWAYS_EXPECTED.contains(&field) || old.get(key) == Some(value) {
            continue;
        }
        let matcher = match field {
            "discover_queue" => Value::Sequence(
                after
                    .discover_queue
                    .iter()
                    .map(|options| unit_matchers(options))
                    .collect(),
            ),
            "pending_combined_choices" => Value::Sequence(
                after
                    .pending_combined_choices
                    .iter()
                    .map(|(a, b)| Value::Sequence(vec![unit_matcher(a), unit_matcher(b)]))
                    .collect(),
            ),
            "auras" => auras_matcher(&before.auras, &after.auras)?,
            _ => changed(old.get(key), value),
        };
        if matcher.as_mapping().is_some_and(Mapping::is_empty) {
            continue;
        }
        expect.insert(key.clone(), matcher);
    }
    expect.insert("board".into(), unit_matchers(&after.board));
    expect.insert("hand".into(), unit_matchers(&after.hand));
    expect.insert("shop".into(), unit_matchers(&after.shop));
    expect.insert(
        "discover_pending".into(),
        after
            .discover_pending
            .as_deref()
            .map_or(Value::Null, unit_matchers),
    );
    if !pool_changes.is_empty() {
        expect.insert("pool".into(), Value::Mapping(pool_changes));
    }
    Ok(Value::Mapping(expect))
}

/// An expectation for the auras fields that changed from `before` to `after` (card counters by
/// card name, effects as a whole).
fn auras_matcher(before: &PlayerAuras, after: &PlayerAuras) -> Result<Value, String> {
    let (old, new) = (to_value(before)?, to_value(after)?);
    let mut expect = Mapping::new();
    for (key, value) in fields(&new)? {
        if old.get(key) == Some(value) {
            continue;
        }
        match key.as_str().unwrap_or_default() {
            "card_counters" => {
                let ids: BTreeSet<CardId> = before
                    .card_counters
                    .keys()
                    .chain(after.card_counters.keys())
                    .copied()
                    .collect();
                let counters: Mapping = ids
                    .into_iter()
                    .filter(|&id| before.counter_pair(id) != after.counter_pair(id))
                    .map(|id| (card_key(id), pair_value(after.counter_pair(id))))
                    .collect();
                if !counters.is_empty() {
                    expect.insert("counters".into(), Value::Mapping(counters));
                }
            }
            "effects" => {
                let effects = after
                    .effects
                    .iter()
                    .map(|effect| {
                        let mut m = Mapping::new();
                        match card_of(effect.card_id) {
                            Some(card) => m.insert("card".into(), card.name.as_str().into()),
                            None => m.insert("card_id".into(), effect.card_id.into()),
                        };
                        m.insert("stacks".into(), effect.stacks.into());
                        m.insert("duration".into(), untag(to_value(&effect.duration)?));
                        Ok(Value::Mapping(m))
                    })
                    .collect::<Result<_, String>>()?;
                expect.insert("effects".into(), Value::Sequence(effects));
            }
            _ => {
                expect.insert(key.clone(), changed(old.get(key), value));
            }
        }
    }
    Ok(Value::Mapping(expect))
}

/// The unit specs of `units`.
fn unit_specs(units: &[Unit]) -> Result<Value, String> {
    units
        .iter()
        .map(unit_spec)
        .collect::<Result<_, _>>()
        .map(Value::Sequence)
}

/// A unit spec that builds exactly `unit`: the first candidate spec string needing the fewest
/// extra fields, with those fields as a mapping (`{card: <spec>, <field>: value, ...}`) if it
/// needs any. Every spec is built back and checked against `unit`.
fn unit_spec(unit: &Unit) -> Result<Value, String> {
    let target = to_value(unit)?;
    let mut candidates = Vec::new();
    for spec in spec_candidates(unit) {
        let built = build_unit(&Value::String(spec.clone()))
            .map_err(|e| format!("cannot build the unit spec {spec:?}: {e}"))?;
        let extra = struct_patch(fields(&to_value(&built)?)?, fields(&target)?)?;
        let spec = if extra.is_empty() {
            Value::String(spec)
        } else {
            let mut m = Mapping::new();
            m.insert("card".into(), Value::String(spec));
            m.extend(extra);
            Value::Mapping(m)
        };
        candidates.push(spec);
    }
    // The fewest extra fields first (a stable sort: the most readable spec among equals).
    candidates.sort_by_key(|spec| spec.as_mapping().map_or(0, Mapping::len));
    let mut wrong = None;
    for spec in candidates {
        match build_unit(&spec) {
            Ok(built) if built == *unit => return Ok(spec),
            Ok(built) => {
                let diff = difference(&to_value(&built)?, &target, "")
                    .unwrap_or_else(|| "they differ".to_string());
                wrong.get_or_insert(format!(
                    "{} builds a different unit: {diff}",
                    yaml::flow(&spec)
                ));
            }
            Err(e) => {
                wrong.get_or_insert(format!("{} does not build: {e}", yaml::flow(&spec)));
            }
        }
    }
    Err(format!(
        "no unit spec builds {} exactly ({})",
        unit.name,
        wrong.unwrap_or_else(|| "no candidates".to_string())
    ))
}

/// Unit spec strings that build something close to `unit`, the most readable first: its card
/// with `golden`, its stats (as `A/H`, and as a buff `+A/+H` of the card's) and the keywords it
/// gained or lost; a vanilla `A/H` for units that are not cards.
fn spec_candidates(unit: &Unit) -> Vec<String> {
    let Some(card) = card_of(unit.card_id) else {
        return vec![format!("{}/{}", unit.attack.max(0), unit.health.max(0))];
    };
    let proto = &card.proto;
    let mut head = String::new();
    if unit.is_golden && !proto.is_golden {
        head.push_str("golden ");
    }
    head.push_str(&card.name);
    if !unit.is_golden && proto.is_golden {
        head.push_str(" -golden");
    }
    let mut keywords = String::new();
    for kw in KEYWORDS {
        match (unit.has_keyword(kw), proto.has_keyword(kw)) {
            (true, false) => keywords.push_str(&format!(" {}", keyword_name(kw))),
            (false, true) => keywords.push_str(&format!(" -{}", keyword_name(kw))),
            _ => {}
        }
    }
    let mut stats = Vec::new();
    let (da, dh) = (unit.attack - proto.attack, unit.health - proto.health);
    if (da, dh) == (0, 0) {
        stats.push(String::new());
    } else {
        if unit.attack >= 0 && unit.health >= 0 {
            stats.push(format!(" {}/{}", unit.attack, unit.health));
        }
        stats.push(format!(" {da:+}/{dh:+}"));
    }
    let mut candidates = Vec::new();
    for keywords in [keywords.as_str(), ""] {
        for stats in &stats {
            let spec = format!("{head}{stats}{keywords}");
            if !candidates.contains(&spec) {
                candidates.push(spec);
            }
        }
    }
    candidates
}

/// The unit matchers of `units`.
fn unit_matchers(units: &[Unit]) -> Value {
    Value::Sequence(units.iter().map(unit_matcher).collect())
}

/// An expectation for a unit: a unit spec with its card, `golden`, stats (not for spells), the
/// keywords it has and `-keyword` for the card's keywords it lost. Units that a spec cannot
/// match (negative stats, units that are not cards) get a mapping of view fields instead.
fn unit_matcher(unit: &Unit) -> Value {
    let card = card_of(unit.card_id);
    match card {
        Some(card) if unit.attack >= 0 && unit.health >= 0 => {
            let mut spec = if unit.is_golden {
                format!("golden {}", card.name)
            } else {
                card.name.clone()
            };
            if !unit.is_spell {
                spec.push_str(&format!(" {}/{}", unit.attack, unit.health));
            }
            for kw in KEYWORDS {
                if unit.has_keyword(kw) {
                    spec.push_str(&format!(" {}", keyword_name(kw)));
                } else if card.proto.has_keyword(kw) {
                    spec.push_str(&format!(" -{}", keyword_name(kw)));
                }
            }
            Value::String(spec)
        }
        _ => {
            let mut m = Mapping::new();
            match card {
                Some(card) => {
                    m.insert("card".into(), card.name.as_str().into());
                }
                None => {
                    m.insert("name".into(), unit.name.as_str().into());
                    m.insert("card_id".into(), unit.card_id.into());
                }
            }
            m.insert(
                "stats".into(),
                format!("{}/{}", unit.attack, unit.health).into(),
            );
            let keywords = KEYWORDS
                .into_iter()
                .filter(|&kw| unit.has_keyword(kw))
                .map(|kw| Value::String(keyword_name(kw)))
                .collect();
            m.insert("keywords".into(), Value::Sequence(keywords));
            m.insert("is_golden".into(), unit.is_golden.into());
            Value::Mapping(m)
        }
    }
}

/// The scenario step performing `action` in `before`.
fn action_step(before: &TavernState, action: TavernAction) -> Value {
    let at = |pairs: &[(&str, usize)]| -> Value {
        Value::Mapping(
            pairs
                .iter()
                .map(|&(key, i)| (Value::from(key), Value::from(i as u64)))
                .collect(),
        )
    };
    let index = |i: usize| Value::from(i as u64);
    match action {
        TavernAction::Buy { shop_index } => single("buy", index(shop_index)),
        TavernAction::Play {
            hand_index,
            board_pos,
        } if board_pos == before.board.len() => single("play", index(hand_index)),
        TavernAction::Play {
            hand_index,
            board_pos,
        } => single("play", at(&[("hand", hand_index), ("pos", board_pos)])),
        TavernAction::Sell { board_pos } => single("sell", index(board_pos)),
        TavernAction::Reposition { from_pos, to_pos } => {
            single("reposition", at(&[("from", from_pos), ("to", to_pos)]))
        }
        TavernAction::Activate {
            board_pos,
            target_pos: None,
        } => single("activate", index(board_pos)),
        TavernAction::Activate {
            board_pos,
            target_pos: Some(target),
        } => single("activate", at(&[("pos", board_pos), ("target", target)])),
        TavernAction::Refresh => "refresh".into(),
        TavernAction::UpgradeTavern => "upgrade_tavern".into(),
        TavernAction::ToggleFreeze => "toggle_freeze".into(),
        TavernAction::ChooseDiscover { option_index } => {
            single("choose_discover", index(option_index))
        }
        TavernAction::EndTurn => "end_turn".into(),
        TavernAction::PlayOnShop {
            hand_index,
            shop_index,
        } => single("play", at(&[("hand", hand_index), ("shop", shop_index)])),
        TavernAction::Pass { hand_index } => single("pass", index(hand_index)),
        TavernAction::HeroPower { target: None } => "hero_power".into(),
        TavernAction::HeroPower { target: Some(t) } => single("hero_power", index(t)),
        TavernAction::HeroPower2 { target: None } => "hero_power_2".into(),
        TavernAction::HeroPower2 { target: Some(t) } => single("hero_power_2", index(t)),
        TavernAction::ChooseHero { option_index } => single("choose_hero", index(option_index)),
        TavernAction::ChooseTrinket { option_index } => {
            single("choose_trinket", index(option_index))
        }
        TavernAction::DarkDiscovery => "dark_discovery".into(),
    }
}

/// An action in words (`"buy Alleycat"`) and as a scenario name part (`buy_alleycat`).
fn describe(action: TavernAction, card: Option<CardId>) -> (String, String) {
    let verb = match action {
        TavernAction::Buy { .. } => "buy",
        TavernAction::Play { .. } => "play",
        TavernAction::PlayOnShop { .. } => "cast",
        TavernAction::Pass { .. } => "pass",
        TavernAction::Sell { .. } => "sell",
        TavernAction::Reposition { .. } => "move",
        TavernAction::Activate { .. } => "activate",
        TavernAction::ChooseDiscover { .. } => "discover",
        TavernAction::Refresh => return ("refresh".into(), "refresh".into()),
        TavernAction::UpgradeTavern => {
            return ("upgrade the Tavern".into(), "upgrade_tavern".into())
        }
        TavernAction::ToggleFreeze => return ("toggle the freeze".into(), "toggle_freeze".into()),
        TavernAction::EndTurn => return ("end the turn".into(), "end_turn".into()),
        TavernAction::HeroPower { .. } => {
            return ("use the Hero Power".into(), "hero_power".into())
        }
        TavernAction::HeroPower2 { .. } => {
            return ("use the second Hero Power".into(), "hero_power_2".into())
        }
        TavernAction::ChooseHero { option_index } => {
            return (
                format!("choose hero option {option_index}"),
                format!("choose_hero_{option_index}"),
            )
        }
        TavernAction::ChooseTrinket { option_index } => {
            return (
                format!("choose trinket option {option_index}"),
                format!("choose_trinket_{option_index}"),
            )
        }
        TavernAction::DarkDiscovery => {
            return ("use Dark Discovery".into(), "dark_discovery".into())
        }
    };
    let phrase = match card {
        Some(card) => format!("{verb} {}", card_name(card)),
        None => verb.to_string(),
    };
    let word = slug(&phrase);
    (phrase, word)
}

/// `<stem>_frame_<n>_<player>_<action>_then_<action>...`, at most [`MAX_NAME`] characters
/// (unless the first action alone is longer).
fn scenario_name(stem: &str, frame: usize, player: &str, words: &[&str]) -> String {
    let mut name = slug(&format!("{stem} frame {frame} {player}"));
    if words.is_empty() {
        name.push_str("_state");
        return name;
    }
    for (i, word) in words.iter().enumerate() {
        let next = format!("{name}{}{word}", if i == 0 { "_" } else { "_then_" });
        let rest = words.len() - i - 1;
        let tail = if rest > 0 {
            format!("_and_{rest}_more").len()
        } else {
            0
        };
        if i > 0 && next.len() + tail > MAX_NAME {
            name.push_str(&format!("_and_{}_more", words.len() - i));
            return name;
        }
        name = next;
    }
    name
}

/// The comment lines at the top of an exported scenario (`missing`: the copies `take_from_pool`
/// takes).
fn header(timeline: &Timeline, source: &str, run: &Run, missing: usize) -> Vec<String> {
    let setup = &timeline.header;
    let (frame, p) = (run.frame, run.p);
    let who = player_name(timeline, p);
    let pools = if run.per_player {
        format!(
            "a pool and RNG per player, {who}'s seed {}",
            setup.players.get(p).map_or(0, |s| s.seed)
        )
    } else {
        format!(
            "a pool and RNG shared by all players, seed {}",
            setup.shared_seed
        )
    };
    let what = match run.actions.len() {
        0 => format!("{who}'s Tavern at frame {frame}"),
        1 => format!("{who}'s Tavern at frame {frame}, then {who}'s next recorded action"),
        n => format!("{who}'s Tavern at frame {frame}, then {who}'s next {n} recorded actions"),
    };
    let pool = if missing > 0 {
        format!(" and `take_from_pool` takes the copies missing from the pool {who} draws from")
    } else {
        format!(" (the pool {who} draws from is full)")
    };
    let mut paragraphs = vec![
        format!(
            "Exported by `match_replay scenario` from {source} (seaglass {}, {} catalog, \
             {pools}).",
            setup.engine, setup.catalog
        ),
        format!(
            "{what}. `tavern:` patches TavernState::new() into {who}'s exact state, `seed:` \
             resumes the RNG {who} draws from{pool}."
        ),
    ];
    if !run.passed.is_empty() {
        let others: Vec<usize> = (0..timeline.num_players())
            .filter(|&q| {
                run.passed
                    .iter()
                    .any(|&i| involves(&timeline.frames[i].kind, q))
            })
            .collect();
        let calls = match others[..] {
            [other] => format!("{}'s calls", player_name(timeline, other)),
            _ => "The other players' calls".to_string(),
        };
        paragraphs.push(format!(
            "{calls} in between ({}) are left out: they cannot touch {who}'s Tavern, pool or RNG \
             (each player has their own pool and RNG).",
            frame_list(&run.passed)
        ));
    }
    if let Some(note) = &run.note {
        paragraphs.push(format!("The scenario {note}."));
    }
    if timeline.engine_mismatch().is_some() {
        paragraphs.push(format!(
            "The replay was recorded by seaglass {}: this is the game as seaglass {} \
             re-simulates it.",
            setup.engine,
            engine_version()
        ));
    }
    if !timeline.is_faithful() {
        paragraphs.push(
            "The replay does not re-simulate faithfully (see `match_replay verify`): this is the \
             game as this build re-simulates it."
                .to_string(),
        );
    }
    paragraphs
        .iter()
        .flat_map(|text| wrap(text, COMMENT_WIDTH))
        .collect()
}

/// `text` in lines of at most `width` characters (longer words get a line of their own).
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.len() + 1 + word.len() > width {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// The file name of `source`.
fn file_name(source: &str) -> String {
    Path::new(source)
        .file_name()
        .map_or_else(|| source.to_string(), |f| f.to_string_lossy().into_owned())
}

/// A replay's file name without `.replay.pb` (or its extension).
fn stem(file: &str) -> String {
    match file.strip_suffix(".replay.pb") {
        Some(stem) => stem.to_string(),
        None => Path::new(file)
            .file_stem()
            .map_or_else(|| file.to_string(), |s| s.to_string_lossy().into_owned()),
    }
}

/// The card `card_id`, if a scenario can name it (its name is not ambiguous).
fn card_of(card_id: CardId) -> Option<&'static Card> {
    let index = card_index();
    let card = index.by_id(card_id)?;
    index
        .resolve(&card.name)
        .ok()
        .filter(|named| named.card_id == card_id)
}

/// The name of card `card_id`, for comments and descriptions.
fn card_name(card_id: CardId) -> String {
    card_index()
        .by_id(card_id)
        .map_or_else(|| format!("card #{card_id}"), |card| card.name.clone())
}

/// A card as a scenario key: its name, or its id if it cannot be named.
fn card_key(card_id: CardId) -> Value {
    match card_of(card_id) {
        Some(card) => card.name.as_str().into(),
        None => card_id.into(),
    }
}

fn pair_value((a, b): (i32, i32)) -> Value {
    Value::Sequence(vec![a.into(), b.into()])
}

/// The snake_case name of a keyword (`divine_shield`).
fn keyword_name(kw: Keyword) -> String {
    match serde_yaml::to_value(kw) {
        Ok(Value::String(name)) => name,
        _ => format!("{kw:?}").to_ascii_lowercase(),
    }
}

/// A single-key mapping (`{buy: 0}`).
fn single(key: &str, value: Value) -> Value {
    let mut m = Mapping::new();
    m.insert(key.into(), value);
    Value::Mapping(m)
}

/// `value` where it differs from `base`: for two structs (mappings of the same fields), the
/// fields that differ (recursively); otherwise all of `value`. Enum variants with data are
/// written as single-key mappings (`{Refreshes: 3}`), as patches and expectations take them.
fn changed(base: Option<&Value>, value: &Value) -> Value {
    if let (Some(Value::Mapping(old)), Value::Mapping(new)) = (base, value) {
        let is_struct = |m: &Mapping| m.keys().all(Value::is_string);
        let same_fields = old.len() == new.len() && new.keys().all(|k| old.contains_key(k));
        if is_struct(old) && is_struct(new) && same_fields {
            let mut diff = Mapping::new();
            for (key, field) in new {
                let before = old.get(key);
                if before != Some(field) {
                    diff.insert(key.clone(), changed(before, field));
                }
            }
            return Value::Mapping(diff);
        }
    }
    untag(value.clone())
}

/// `value` with every YAML tag (`!Refreshes 3`, an enum variant with data) as a single-key
/// mapping (`{Refreshes: 3}`).
fn untag(value: Value) -> Value {
    match value {
        Value::Tagged(tagged) => single(
            tagged.tag.to_string().trim_start_matches('!'),
            untag(tagged.value),
        ),
        Value::Sequence(items) => Value::Sequence(items.into_iter().map(untag).collect()),
        Value::Mapping(m) => Value::Mapping(m.into_iter().map(|(k, v)| (k, untag(v))).collect()),
        other => other,
    }
}

fn to_value(value: &impl Serialize) -> Result<Value, String> {
    serde_yaml::to_value(value).map_err(|e| e.to_string())
}

/// The engine value whose serde form is `value`.
fn from_value<T: DeserializeOwned>(value: &Value) -> Result<T, String> {
    serde_yaml::from_value(value.clone()).map_err(|e| e.to_string())
}

/// The entries of a serialized struct.
fn fields(value: &Value) -> Result<&Mapping, String> {
    value
        .as_mapping()
        .ok_or_else(|| "an engine struct does not serialize to a mapping".to_string())
}

/// `"1 action"`, `"2 actions"`, `"3 copies"`.
fn plural(n: usize, word: &str) -> String {
    match (n, word.strip_suffix('y')) {
        (1, _) => format!("1 {word}"),
        (_, Some(stem)) => format!("{n} {stem}ies"),
        _ => format!("{n} {word}s"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use seaglass::{full_catalog, DeityKind, HeroId, Match, MatchSetup, PlayerSetup, PoolMode};
    use seaglass_tools::bot::choose_semi_random_action;

    /// Player `p`'s hero in a test game played with seed `seed`: one of the heroes a Solo game
    /// with Dark Gifts offers, a different one for each seed and player.
    fn hero(seed: u64, p: u64) -> Option<HeroId> {
        let heroes = heroes::offerable(false, true);
        Some(heroes[((seed * 7 + p * 31) % heroes.len() as u64) as usize])
    }

    /// `sim_match`'s bot, which also picks a trinket when it is offered some and, one time in
    /// four, uses its Hero Power or the Dark Discovery button when it can.
    fn semi_random_action(state: &TavernState, bot: &mut Rng) -> TavernAction {
        if !state.trinket_options.is_empty() {
            let option_index = bot.below(state.trinket_options.len());
            return TavernAction::ChooseTrinket { option_index };
        }
        let powers: Vec<TavernAction> = state
            .valid_actions()
            .into_iter()
            .filter(|a| {
                matches!(
                    a,
                    TavernAction::HeroPower { .. }
                        | TavernAction::HeroPower2 { .. }
                        | TavernAction::DarkDiscovery
                )
            })
            .collect();
        if !powers.is_empty() && bot.below(4) == 0 {
            return powers[bot.below(powers.len())];
        }
        choose_semi_random_action(state, bot)
    }

    /// The match a `sim_match --seed <seed>` run plays, with heroes, trinkets and Dark Gifts: two
    /// semi-random bots ([`semi_random_action`]) sharing a pool and an RNG.
    fn shared_game(seed: u64) -> Timeline {
        let player = |name: &str, p: u64, deity| PlayerSetup {
            name: name.to_string(),
            deity,
            armor: 5,
            shop_spells: true,
            hero: hero(seed, p),
            ..Default::default()
        };
        let setup = MatchSetup {
            pool: PoolMode::Shared,
            shared_seed: seed,
            players: vec![
                player("P1", 0, DeityKind::CThun),
                player("P2", 1, DeityKind::YShaarj),
            ],
            ..Default::default()
        };
        let mut game = Match::new(&setup).unwrap();
        let mut bot = Rng::new(seed ^ 0xB075_EED5_0F5E_A61A);
        for _ in 1..=20 {
            game.set_pairs(&[(0, 1)]);
            game.start_turn(0);
            game.start_turn(1);
            game.deliver_gifts();
            for p in 0..2 {
                for step in 1..=40 {
                    let state = game.player(p);
                    let action = if step == 40 && state.discover_pending.is_none() {
                        TavernAction::EndTurn
                    } else {
                        semi_random_action(state, &mut bot)
                    };
                    if game.step(p, action) != Ok(false) {
                        break;
                    }
                }
            }
            game.deliver_gifts();
            game.combat(0, 1);
            if game.record_deaths().contains(&false) {
                break;
            }
        }
        game.finish();
        timeline(&game)
    }

    /// How the two players of a [`per_player_game`] take turns to act.
    #[derive(Clone, Copy)]
    enum Interleave {
        /// As `rl/seaglass_rl/train_ppo.py`'s self-play plays: every player still in their turn
        /// makes one action per round, P1 then P2, until both have ended their turn.
        Lockstep,
        /// A random player still in their turn makes the next action.
        Random,
    }

    /// The most actions a player makes in a turn of a [`per_player_game`] (`train_ppo.py`'s
    /// `max_steps_per_turn`): after the last one, they pick the first option of every discover
    /// and trinket offer and end their turn.
    const MAX_STEPS: usize = 15;

    /// Two players with their own pools and RNGs (and various heroes, deities, Armor and shop
    /// spells) making uniformly random legal actions (Hero Powers, trinket picks and Dark
    /// Discoveries included), interleaved as `interleave` says.
    fn per_player_game(seed: u64, interleave: Interleave) -> Timeline {
        let deities = [DeityKind::None, DeityKind::CThun, DeityKind::YShaarj];
        let players = (0..2u64)
            .map(|p| PlayerSetup {
                name: String::new(),
                seed: seed * 1000 + p,
                deity: deities[((seed + p) % 3) as usize],
                armor: (seed % 4) as i32 * 2,
                shop_spells: (seed + p).is_multiple_of(2),
                hero: hero(seed, p),
                ..Default::default()
            })
            .collect();
        let setup = MatchSetup {
            pool: PoolMode::PerPlayer,
            players,
            ..Default::default()
        };
        let mut game = Match::new(&setup).unwrap();
        let mut bot = Rng::new(seed);
        for _ in 1..=14 {
            game.set_pairs(&[(0, 1)]);
            game.start_turn(0);
            game.start_turn(1);
            game.deliver_gifts();
            let mut active = vec![0, 1];
            let mut taken = [0; 2];
            while !active.is_empty() {
                let movers = match interleave {
                    Interleave::Lockstep => active.clone(),
                    Interleave::Random => vec![active[bot.below(active.len())]],
                };
                for p in movers {
                    let actions = game.player(p).valid_actions();
                    let action = if actions.is_empty() {
                        TavernAction::EndTurn
                    } else {
                        actions[bot.below(actions.len())]
                    };
                    let mut ended = game.step(p, action).expect("valid actions are legal");
                    taken[p] += 1;
                    if !ended && taken[p] >= MAX_STEPS {
                        loop {
                            let state = game.player(p);
                            let first = if state.discover_pending.is_some() {
                                TavernAction::ChooseDiscover { option_index: 0 }
                            } else if !state.trinket_options.is_empty() {
                                TavernAction::ChooseTrinket { option_index: 0 }
                            } else {
                                break;
                            };
                            game.step(p, first).expect("offers have options");
                        }
                        ended = game.step(p, TavernAction::EndTurn) == Ok(true);
                        assert!(ended, "player {p} cannot end their turn");
                    }
                    if ended {
                        active.retain(|&q| q != p);
                    }
                }
            }
            game.deliver_gifts();
            game.combat(0, 1);
            if game.record_deaths().contains(&false) {
                break;
            }
        }
        game.finish();
        timeline(&game)
    }

    fn timeline(game: &Match) -> Timeline {
        let timeline = Timeline::new(&game.replay()).unwrap();
        assert!(timeline.is_faithful(), "{:?}", timeline.divergences);
        timeline
    }

    /// The frames of player `p`'s next actions (at most `steps`) after frame `frame` that an
    /// export replays, and the frame it stops at (`timeline.len()`: the end of the replay). With a
    /// pool and RNG per player, it passes over the frames that do not involve `p`; with a shared
    /// pool and RNG, it stops at any frame that is not `p`'s action.
    fn replayable(
        timeline: &Timeline,
        frame: usize,
        steps: usize,
        p: usize,
    ) -> (Vec<usize>, usize) {
        let per_player = timeline.header.pool() == pb::PoolMode::PerPlayer;
        let mut frames = Vec::new();
        let mut i = frame + 1;
        while frames.len() < steps && i < timeline.len() {
            if acts(timeline, i, Some(p)) {
                frames.push(i);
            } else if !per_player || involves(&timeline.frames[i].kind, p) {
                break;
            }
            i += 1;
        }
        (frames, i)
    }

    /// The frames of the actions an export replays.
    fn replayed(exported: &Export) -> Vec<usize> {
        exported.expected.checkpoints[1..]
            .iter()
            .map(|c| c.frame)
            .collect()
    }

    /// The comment lines at the top of an export, joined.
    fn header_text(exported: &Export) -> String {
        let lines: Vec<&str> = exported
            .yaml
            .lines()
            .map_while(|l| l.strip_prefix("# "))
            .collect();
        lines.join(" ")
    }

    /// Export player `player`'s next `steps` actions after frame `frame` (by default, whoever acts
    /// next) and check the scenario: it must replay exactly the actions [`replayable`] says (and
    /// say why if they are fewer than `steps`), or fail if there are none. Returns their frames.
    fn check_run(
        timeline: &Timeline,
        source: &str,
        frame: usize,
        steps: usize,
        player: Option<usize>,
    ) -> Vec<usize> {
        let at = format!("{source} frame {frame}, {steps} steps");
        let p = player
            .or_else(|| actor(timeline, frame + 1))
            .expect("an action follows");
        let (frames, stop) = replayable(timeline, frame, steps, p);
        let exported =
            export(timeline, source, frame, steps, player).and_then(|e| e.check().map(|()| e));
        let exported = match exported {
            Ok(exported) => exported,
            Err(e) if frames.is_empty() => {
                if stop < timeline.len() {
                    assert!(e.starts_with(&format!("frame {stop} ")), "{at}: {e}");
                }
                return frames;
            }
            Err(e) => panic!("{at}: {e}"),
        };
        assert!(!frames.is_empty(), "{at}: an export without an action");
        assert_eq!(replayed(&exported), frames, "{at}");
        assert_eq!(exported.actions, frames.len(), "{at}");
        match &exported.note {
            None => assert_eq!(frames.len(), steps, "{at}"),
            Some(note) => {
                let reason = if stop < timeline.len() {
                    format!("frame {stop} ")
                } else {
                    format!("the replay ends at frame {}", timeline.len() - 1)
                };
                let asked = format!(
                    "replays {} of the {steps} actions asked for: {reason}",
                    frames.len()
                );
                assert!(note.starts_with(&asked), "{at}: {note}");
                // Only with a shared pool does an export stop at the other player's calls.
                let other = stop < timeline.len() && !involves(&timeline.frames[stop].kind, p);
                assert_eq!(
                    note.contains("another player's call"),
                    other,
                    "{at}: {note}"
                );
            }
        }
        // The header names the other players' calls the export passes over, if any.
        let last = frames[frames.len() - 1];
        let passed: Vec<usize> = (frame + 1..last).filter(|i| !frames.contains(i)).collect();
        let header = header_text(&exported);
        assert_eq!(
            header.contains(&format!(
                "calls in between ({}) are left out",
                frame_list(&passed)
            )),
            !passed.is_empty(),
            "{at}:\n{}",
            exported.yaml
        );
        if let Some(long) = exported.yaml.lines().find(|l| l.len() > 120) {
            panic!("{at}: a long line: {long}");
        }
        frames
    }

    /// How many scenarios [`export_game`] exported and checked.
    #[derive(Debug, Default)]
    struct Exports {
        /// Scenarios replaying one action, one for every action.
        single: usize,
        /// Longer runs of actions asked for.
        longer: usize,
        /// Longer runs that replay more than one action.
        several: usize,
        /// Longer runs that pass over the other player's calls.
        passing: usize,
        /// Single-action scenarios using a Hero Power (either).
        hero_powers: usize,
        /// Single-action scenarios picking a trinket.
        trinkets: usize,
        /// Single-action scenarios using the Dark Discovery button.
        dark_discoveries: usize,
    }

    impl Exports {
        fn add(self, other: Exports) -> Exports {
            Exports {
                single: self.single + other.single,
                longer: self.longer + other.longer,
                several: self.several + other.several,
                passing: self.passing + other.passing,
                hero_powers: self.hero_powers + other.hero_powers,
                trinkets: self.trinkets + other.trinkets,
                dark_discoveries: self.dark_discoveries + other.dark_discoveries,
            }
        }
    }

    /// [`check_run`] every action of a game (one action each, from the frame before it) and
    /// longer runs of actions: from the frame before every sixth action (as many as the player
    /// can replay, before every 31st), and from the frame after others (where the other player
    /// may act next).
    fn export_game(source: &str, timeline: &Timeline) -> Exports {
        let mut exports = Exports::default();
        for i in 1..timeline.len() {
            let Some(player) = actor(timeline, i) else {
                continue;
            };
            assert_eq!(check_run(timeline, source, i - 1, 1, None), [i]);
            exports.single += 1;
            if let FrameKind::Act { action, .. } = timeline.frames[i].kind {
                match action {
                    TavernAction::HeroPower { .. } | TavernAction::HeroPower2 { .. } => {
                        exports.hero_powers += 1
                    }
                    TavernAction::ChooseTrinket { .. } => exports.trinkets += 1,
                    TavernAction::DarkDiscovery => exports.dark_discoveries += 1,
                    _ => {}
                }
            }
            let runs = [
                (i % 6 == 0, i - 1, 2 + i % 7),
                (i % 31 == 0, i - 1, 1000),
                (i % 6 == 3, i, 2 + i % 3),
            ];
            for (_, frame, steps) in runs.into_iter().filter(|run| run.0) {
                let frames = check_run(timeline, source, frame, steps, Some(player));
                exports.longer += 1;
                exports.several += usize::from(frames.len() > 1);
                exports.passing += usize::from(
                    frames
                        .last()
                        .is_some_and(|&last| last > frame + frames.len()),
                );
            }
        }
        exports
    }

    /// [`export_game`] every game (its name, and how to play it), one thread each; returns the
    /// total counts.
    fn export_games<F>(games: impl Iterator<Item = (String, F)>) -> Exports
    where
        F: FnOnce() -> Timeline + Send,
    {
        std::thread::scope(|scope| {
            let threads: Vec<_> = games
                .map(|(name, play)| scope.spawn(move || export_game(&name, &play())))
                .collect();
            threads
                .into_iter()
                .map(|thread| {
                    thread
                        .join()
                        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
                })
                .fold(Exports::default(), Exports::add)
        })
    }

    #[test]
    fn every_action_of_shared_pool_games_exports_a_passing_scenario() {
        let games = (1..=6).map(|seed| {
            let name = format!("replays/s{seed}.replay.pb");
            (name, move || shared_game(seed))
        });
        let exports = export_games(games);
        eprintln!("shared pool, 6 games: {exports:?}");
        assert!(exports.single > 300, "{exports:?}");
        assert!(exports.several > 0, "{exports:?}");
        // The heroes, trinkets and Dark Gifts are exported too.
        assert!(exports.hero_powers > 0, "{exports:?}");
        assert!(exports.trinkets > 0, "{exports:?}");
        assert!(exports.dark_discoveries > 0, "{exports:?}");
        // With one pool and RNG, every other player's call stops an export.
        assert_eq!(exports.passing, 0, "{exports:?}");
    }

    #[test]
    fn every_action_of_per_player_games_exports_a_passing_scenario() {
        // Six games in which the players alternate actions (as train_ppo.py's self-play records
        // them), and four in which a random player acts next.
        let games = (1..=10).map(|seed| {
            let (interleave, order) = if seed <= 6 {
                (Interleave::Lockstep, "lockstep")
            } else {
                (Interleave::Random, "random_order")
            };
            let name = format!("{order}_{seed}.replay.pb");
            (name, move || per_player_game(seed, interleave))
        });
        let exports = export_games(games);
        eprintln!("per-player pools, 10 games: {exports:?}");
        assert!(exports.single > 500, "{exports:?}");
        assert!(exports.hero_powers > 0, "{exports:?}");
        assert!(exports.trinkets > 0, "{exports:?}");
        assert!(exports.dark_discoveries > 0, "{exports:?}");
        // Most longer runs replay several actions, and many pass over the other player's calls.
        assert!(exports.several * 2 > exports.longer, "{exports:?}");
        assert!(exports.passing * 4 > exports.longer, "{exports:?}");
    }

    #[test]
    fn per_player_exports_pass_over_the_other_players_calls() {
        let timeline = per_player_game(3, Interleave::Lockstep);
        let g = "g.replay.pb";
        // An action followed by the other player's, by a player with more actions to come.
        let (i, p) = (1..timeline.len() - 1)
            .find_map(|i| {
                let p = actor(&timeline, i)?;
                let more = replayable(&timeline, i - 1, 1000, p).0.len() >= 3;
                (actor(&timeline, i + 1) == Some(1 - p) && more).then_some((i, p))
            })
            .expect("interleaved actions");
        let (who, other) = (player_name(&timeline, p), player_name(&timeline, 1 - p));
        let (frames, stop) = replayable(&timeline, i - 1, 1000, p);
        let last = frames[frames.len() - 1];
        let exported = export(&timeline, g, i - 1, 1000, Some(p)).unwrap();
        exported.check().unwrap();
        assert_eq!(replayed(&exported), frames);
        // It replays the rest of the player's turn, and stops at the lobby's call after it (the
        // gifts delivered once both players ended their turn), which can change any Tavern.
        assert!(matches!(
            timeline.frames[last].kind,
            FrameKind::Act {
                action: TavernAction::EndTurn,
                ..
            }
        ));
        assert!(matches!(
            timeline.frames[stop].kind,
            FrameKind::DeliverGifts
        ));
        let title = frame_title(&timeline, stop);
        let note = format!(
            "replays {} of the 1000 actions asked for: frame {stop} is not an action by {who} \
             ({title})",
            frames.len()
        );
        assert_eq!(exported.note, Some(note));
        let passed: Vec<usize> = (i..last).filter(|j| !frames.contains(j)).collect();
        assert!(passed.iter().all(|&j| actor(&timeline, j) == Some(1 - p)));
        let left_out = format!(
            "{other}'s calls in between ({}) are left out: they cannot touch {who}'s Tavern, pool \
             or RNG (each player has their own pool and RNG).",
            frame_list(&passed)
        );
        assert!(
            header_text(&exported).contains(&left_out),
            "{}",
            exported.yaml
        );
        for frame in &frames {
            assert!(
                exported.yaml.contains(&format!("  # frame {frame}")),
                "{}",
                exported.yaml
            );
        }
        // From the frame before the other player's action: the player's next action...
        let exported = export(&timeline, g, i, 1, Some(p)).unwrap();
        exported.check().unwrap();
        assert_eq!(replayed(&exported), [frames[1]]);
        assert_eq!(exported.note, None);
        // ... or, by default, the other player's.
        assert_eq!(
            replayed(&export(&timeline, g, i, 1, None).unwrap()),
            [i + 1]
        );
        // After the end of the player's turn, the next call involving them is the lobby's.
        let err = export(&timeline, g, last, 1, Some(p)).err().unwrap();
        assert!(
            err.starts_with(&format!("frame {stop} is not an action by {who} ({title}); the next action by {who} is frame ")),
            "{err}"
        );
    }

    #[test]
    fn the_scenario_reads_like_a_hand_written_one() {
        let timeline = shared_game(7);
        let i = (1..timeline.len())
            .find(|&i| {
                matches!(
                    timeline.frames[i].kind,
                    FrameKind::Act {
                        action: TavernAction::Buy { .. },
                        ..
                    }
                ) && timeline.frames[i].turn >= 3
            })
            .unwrap();
        let FrameKind::Act {
            player,
            action: TavernAction::Buy { shop_index },
            card_id: Some(card),
            ..
        } = timeline.frames[i].kind
        else {
            unreachable!()
        };
        let exported = export(&timeline, "replays/s7.replay.pb", i - 1, 1, None).unwrap();
        exported.check().unwrap();
        let yaml = &exported.yaml;
        let who = player_name(&timeline, player);
        let name = slug(&format!("s7 frame {} {who} buy {}", i - 1, card_name(card)));
        assert_eq!(exported.name, name);
        for line in [
            "# Exported by `match_replay scenario` from replays/s7.replay.pb".to_string(),
            format!("topic: replay s7.replay.pb frame {} ({who})", i - 1),
            "scenarios:".to_string(),
            format!("  - name: {name}"),
            "    tavern:".to_string(),
            "    steps:".to_string(),
            format!(
                "      - buy: {shop_index}  # frame {i}: {}",
                card_name(card)
            ),
            "    expect:".to_string(),
            "      board:".to_string(),
        ] {
            assert!(
                yaml.lines().any(|l| l.starts_with(&line)),
                "no line starting with {line:?} in\n{yaml}"
            );
        }
        assert!(yaml.lines().all(|l| l.len() <= 120), "{yaml}");
    }

    #[test]
    fn shared_pool_exports_stop_at_the_other_players_calls() {
        let timeline = shared_game(4);
        let i = (1..timeline.len())
            .find(|&i| actor(&timeline, i).is_some())
            .unwrap();
        let p = actor(&timeline, i).unwrap();
        let exported = export(&timeline, "s4.replay.pb", i - 1, 1000, None).unwrap();
        exported.check().unwrap();
        assert!(exported.actions >= 1 && exported.actions < 1000);
        // The player makes their whole turn, then the other player acts: as they draw from the
        // same pool and RNG, the export stops there.
        let stop = i + exported.actions;
        assert_eq!(actor(&timeline, stop), Some(1 - p));
        let note = exported.note.as_deref().unwrap();
        assert_eq!(
            note,
            format!(
                "replays {} of the 1000 actions asked for: frame {stop} is another player's call \
                 ({}), and all players draw from one pool and RNG",
                exported.actions,
                frame_title(&timeline, stop)
            )
        );
        assert!(
            header_text(&exported).contains(&format!("The scenario {note}.")),
            "{}",
            exported.yaml
        );
    }

    #[test]
    fn the_check_fails_loudly_when_an_export_is_wrong() {
        let timeline = shared_game(5);
        let i = (1..timeline.len())
            .find(|&i| actor(&timeline, i).is_some())
            .unwrap();
        let fresh = || export(&timeline, "s5.replay.pb", i - 1, 1, None).unwrap();
        fresh().check().unwrap();
        // A setup that does not rebuild the replay.
        let mut wrong = fresh();
        wrong.expected.seed ^= 1;
        let err = wrong.check().unwrap_err();
        assert!(
            err.starts_with(&format!(
                "the exported scenario does not rebuild the replay exactly (a bug in \
                 `match_replay scenario`): the RNG at frame {} is in state",
                i - 1
            )),
            "{err}"
        );
        // A pool that is not the replay's.
        let mut wrong = fresh();
        let card = wrong.expected.taken.pop().expect("the shops took copies");
        let err = wrong.check().unwrap_err();
        assert!(
            err.contains(&format!(
                "the pool at frame {} is not the replay's: ",
                i - 1
            )) && err.contains(&card_name(card)),
            "{err}"
        );
        // A scenario that does not pass.
        let mut wrong = fresh();
        let key = "    expect:\n      gold: ";
        let at = wrong.yaml.find(key).unwrap() + key.len();
        let end = at + wrong.yaml[at..].find('\n').unwrap();
        let gold: i64 = wrong.yaml[at..end].parse().unwrap();
        wrong.yaml.replace_range(at..end, &(gold + 1).to_string());
        let err = wrong.check().unwrap_err();
        assert!(
            err.starts_with("the exported scenario does not pass: scenario "),
            "{err}"
        );
        assert!(err.contains("gold"), "{err}");
    }

    #[test]
    fn zero_steps_rebuild_any_players_state_at_any_frame() {
        let timeline = per_player_game(2, Interleave::Lockstep);
        let last = timeline.len() - 1;
        for frame in [0, 1, last / 3, last / 2, last] {
            for p in 0..2 {
                let exported = export(&timeline, "g.replay.pb", frame, 0, Some(p)).unwrap();
                exported.check().unwrap();
                assert_eq!(exported.actions, 0);
                assert!(exported.name.ends_with("_state"), "{}", exported.name);
            }
        }
    }

    #[test]
    fn frames_without_an_action_are_explained() {
        let timeline = shared_game(3);
        let last = timeline.len() - 1;
        let err = export(&timeline, "g", last, 1, None).err().unwrap();
        assert!(
            err.starts_with(&format!("frame {last} is the last frame")),
            "{err}"
        );
        // Frame 1 is the lobby pairing the players for turn 1, frame 2 P1 starting turn 1.
        assert!(matches!(
            timeline.frames[1].kind,
            FrameKind::SetPairs { .. }
        ));
        let first = (1..timeline.len())
            .find(|&i| actor(&timeline, i).is_some())
            .unwrap();
        for (frame, title) in [
            (0, frame_title(&timeline, 1)),
            (1, "P1 starts turn 1".into()),
        ] {
            let err = export(&timeline, "g", frame, 1, None).err().unwrap();
            assert!(
                err.starts_with(&format!("frame {} is not an action ({title})", frame + 1))
                    && err.contains(&format!("use --frame {}", first - 1)),
                "{err}"
            );
        }
        let err = export(&timeline, "g", 0, 0, None).err().unwrap();
        assert!(err.contains("--player"), "{err}");
        // The other player's action: with one pool and RNG, it stops the export.
        let p = actor(&timeline, first).unwrap();
        let other = player_name(&timeline, 1 - p);
        let err = export(&timeline, "g", first - 1, 1, Some(1 - p))
            .err()
            .unwrap();
        assert!(
            err.starts_with(&format!(
                "frame {first} is another player's call ({}), and all players draw from one pool \
                 and RNG; the next action by {other} is frame ",
                frame_title(&timeline, first)
            )),
            "{err}"
        );
    }

    #[test]
    fn players_are_named_or_numbered() {
        let timeline = shared_game(1);
        assert_eq!(parse_player(&timeline, "P2"), Ok(1));
        assert_eq!(parse_player(&timeline, "p1"), Ok(0));
        assert_eq!(parse_player(&timeline, "1"), Ok(1));
        let err = parse_player(&timeline, "P3").unwrap_err();
        assert_eq!(err, "no player \"P3\" (players: P1 (0), P2 (1))");
    }

    #[test]
    fn unit_specs_build_their_units_exactly() {
        let name = full_catalog()[0].name.clone();
        let mut unit = card_index().resolve(&name).unwrap().proto.clone();
        assert_eq!(unit_spec(&unit), Ok(Value::String(name.clone())));
        unit.add_stats(2, 1);
        assert_eq!(unit_spec(&unit), Ok(Value::String(format!("{name} +2/+1"))));
        unit.apply_keyword(Keyword::Taunt, false);
        unit.is_golden = true;
        unit.charges = 3;
        let spec = unit_spec(&unit).unwrap();
        assert_eq!(build_unit(&spec), Ok(unit.clone()), "{spec:?}");
        let mut vanilla = Unit::new("Mystery", -1, 4);
        vanilla.card_id = 999_999;
        let spec = unit_spec(&vanilla).unwrap();
        assert_eq!(build_unit(&spec), Ok(vanilla), "{spec:?}");
    }

    #[test]
    fn names_stay_short() {
        let words = ["buy_alleycat"; 20];
        let name = scenario_name("s7", 55, "P1", &words);
        assert!(name.len() <= MAX_NAME, "{name}");
        assert!(
            name.starts_with("s7_frame_55_p1_buy_alleycat_then_buy_alleycat"),
            "{name}"
        );
        assert!(name.ends_with("_more"), "{name}");
        assert_eq!(scenario_name("s7", 55, "P1", &[]), "s7_frame_55_p1_state");
        assert_eq!(plural(3, "copy"), "3 copies");
        assert_eq!(plural(1, "action"), "1 action");
    }
}
