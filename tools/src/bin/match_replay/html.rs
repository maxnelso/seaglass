//! `match_replay html`: one self-contained HTML page for stepping through a replay
//! (`engine/docs/replays.md`).
//!
//! The page is `tools/viewer/replay.html` with the re-simulated replay embedded as JSON
//! ([`ViewerData`]). The JSON only holds what the page shows, and stays small: every distinct
//! unit appears once in `units` and is referenced by index, and a frame only holds the player
//! states its call changed (the page carries the others forward).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::Serialize;

use seaglass::cards::{self, spells};
use seaglass::replay::{self, pb, FrameKind};
use seaglass::{
    base_copies_for_tier, ActivateTargetKind, BattleOutcome, BattleResult, Event, Side,
    TavernAction, TavernState, Timeline, Tribe, Unit,
};
use seaglass::{heroes, trinkets};
use seaglass_tools::fmt::{diff_auras, format_deity_kind, format_tribe, CombatNames, PoolCounts};
use seaglass_tools::replay_text::{frame_pool, frame_title, note_text, player_name};

/// The viewer page. [`DATA`] marks where the replay's JSON goes.
const TEMPLATE: &str = include_str!("../../../viewer/replay.html");
const DATA: &str = "__REPLAY_DATA__";

/// A combat uses at most this many units of each board.
const MAX_BOARD_SIZE: usize = 7;

/// `match_replay html <file> [-o out.html]`.
pub fn run(args: &[String]) -> Result<ExitCode, String> {
    let timeline = crate::load_one(args)?;
    let input = Path::new(&args[0]);
    let output = match crate::option(args, &["-o", "--output"]) {
        Some(path) => PathBuf::from(path),
        None if args.iter().any(|a| a == "-o" || a == "--output") => {
            return Err("-o needs an output file".to_string());
        }
        None => default_output(input),
    };
    let source = input.file_name().map_or_else(
        || args[0].clone(),
        |name| name.to_string_lossy().into_owned(),
    );
    let page = render(&timeline, &source);
    if let Some(dir) = output.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&output, &page).map_err(|e| format!("{}: {e}", output.display()))?;
    println!(
        "wrote {} ({}, {} frames)",
        output.display(),
        file_size(page.len()),
        timeline.len()
    );
    Ok(ExitCode::SUCCESS)
}

/// The page's default path: the replay's, ending in `.html` instead of `.replay.pb` (or instead
/// of its extension).
fn default_output(input: &Path) -> PathBuf {
    let name = input.file_name().map(|n| n.to_string_lossy());
    match name.as_deref().and_then(|n| n.strip_suffix(".replay.pb")) {
        Some(stem) if !stem.is_empty() => input.with_file_name(format!("{stem}.html")),
        _ => input.with_extension("html"),
    }
}

fn file_size(bytes: usize) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    if (bytes as f64) < MB {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / MB)
    }
}

/// The viewer page of `timeline`. `source` names the replay file on the page.
pub fn render(timeline: &Timeline, source: &str) -> String {
    let json = serde_json::to_string(&ViewerData::new(timeline, source))
        .expect("the viewer data is plain JSON");
    // Without any `<`, the JSON can neither end its <script> element (`</script>`) nor open an
    // HTML comment in it (`<!--`). `<` only occurs inside JSON strings, where `\u003c` is the same.
    TEMPLATE.replacen(DATA, &json.replace('<', "\\u003c"), 1)
}

/// Everything the page shows (the `DATA` object of `tools/viewer/replay.html`).
#[derive(Serialize)]
struct ViewerData<'a> {
    /// The replay's file name.
    source: &'a str,
    setup: SetupView<'a>,
    /// The engine build that rendered the page ([`replay::engine_version`]).
    viewer: &'static str,
    /// The build that recorded the replay, if it is not `viewer` ([`Timeline::engine_mismatch`]).
    engine_mismatch: Option<&'a str>,
    /// Whether every call re-simulated and matched its recording ([`Timeline::is_faithful`]).
    faithful: bool,
    divergences: Vec<DivergenceView<'a>>,
    /// The call the re-simulation stopped at ([`Timeline::error`]), right after the last frame.
    error: Option<ErrorView<'a>>,
    /// The result (the title of the `End` frame), if the replay gets there.
    result: Option<String>,
    actions: usize,
    turns: u32,
    /// Every distinct unit, referenced by its index.
    units: Vec<UnitView>,
    /// One per timeline frame.
    frames: Vec<FrameView>,
}

#[derive(Serialize)]
struct SetupView<'a> {
    engine: &'a str,
    format: u32,
    catalog: &'a str,
    /// `shared` or `per-player`.
    pool: &'static str,
    /// The seed of the shared RNG (shared pools only). Seeds are strings: JSON numbers lose
    /// precision past 2^53.
    shared_seed: Option<String>,
    players: Vec<PlayerSetupView>,
    meta: &'a BTreeMap<String, String>,
}

#[derive(Serialize)]
struct PlayerSetupView {
    name: String,
    deity: &'static str,
    armor: i32,
    shop_spells: bool,
    /// The seed of the player's RNG (per-player pools only).
    seed: Option<String>,
}

#[derive(Serialize)]
struct DivergenceView<'a> {
    record: usize,
    frame: usize,
    message: &'a str,
}

#[derive(Serialize)]
struct ErrorView<'a> {
    record: usize,
    message: &'a str,
}

/// One frame: the call that produced it and what it changed.
#[derive(Serialize)]
struct FrameView {
    /// `start`, `turn` (a player started a turn), `act`, `combat` or `end`.
    kind: &'static str,
    /// The highest turn any player has reached.
    turn: u32,
    /// The replay record the frame re-simulates.
    #[serde(skip_serializing_if = "Option::is_none")]
    record: Option<usize>,
    /// What happened, worded like `match_replay log` ([`frame_title`], without the note).
    title: String,
    /// A shorter title for the frame list (actions only).
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    /// Who acted: the actor, both fighters, or nobody.
    actors: Vec<usize>,
    /// Each player's state if it changed since the previous frame (always in frame 0), else `null`.
    states: Vec<Option<PlayerView>>,
    /// Each player's pool counts if they changed since the previous frame, else `null`.
    pools: Vec<Option<PoolView>>,
    /// `[player, changes]` for each player whose auras changed, worded like `match_replay log`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    auras: Vec<(usize, Vec<String>)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<NoteView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    combat: Option<CombatView>,
    /// The winner of an `end` frame (none for a draw).
    #[serde(skip_serializing_if = "Option::is_none")]
    winner: Option<usize>,
    /// How this frame's call differs from the recorded one ([`Timeline::divergences`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    divergences: Vec<String>,
}

/// What the page shows of a player's state. Units are indices into [`ViewerData::units`].
#[derive(Serialize, Clone, PartialEq)]
struct PlayerView {
    turn: u32,
    health: i32,
    armor: i32,
    tier: u32,
    gold: u32,
    max_gold: u32,
    /// Extra Gold next turn.
    bonus_gold: u32,
    /// The cost of the next Tavern upgrade (`null` at the top).
    upgrade: Option<u32>,
    frozen: bool,
    deity: DeityView,
    spells_cast: u32,
    spell_buff: [i32; 2],
    blood_gem: [i32; 2],
    undead_attack: i32,
    free_refreshes: u32,
    board: Vec<usize>,
    hand: Vec<usize>,
    shop: Vec<usize>,
    /// The pending Discover options.
    discover: Option<Vec<usize>>,
    /// Discover prompts queued after the pending one.
    queued: usize,
}

#[derive(Serialize, Clone, PartialEq)]
struct DeityView {
    kind: &'static str,
    attack: i32,
    health: i32,
    golden: bool,
}

/// `[copies left, base copies]` for each shop card and Discover option drawn from the pool
/// (`[pool 14/15]` in `match_replay log`), `null` for the others. Separate from [`PlayerView`]
/// because another player's buys change it.
#[derive(Serialize, Clone, PartialEq)]
struct PoolView {
    shop: Vec<Option<[u32; 2]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    discover: Option<Vec<Option<[u32; 2]>>>,
}

/// What the page shows of a unit.
#[derive(Serialize, Clone, PartialEq, Eq, Hash)]
struct UnitView {
    name: String,
    card: u32,
    tier: u32,
    attack: i32,
    health: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    tribe: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    keywords: Vec<&'static str>,
    #[serde(skip_serializing_if = "is_false")]
    golden: bool,
    /// What a spell costs to buy (`1g`, `2 HP`). Only spells have it.
    #[serde(skip_serializing_if = "Option::is_none")]
    spell: Option<String>,
    /// The Gold cost of the unit's Activate ability, if it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    activate: Option<u32>,
    /// The Activate ability was used this turn.
    #[serde(skip_serializing_if = "is_false")]
    activated: bool,
    /// An awakened Deity.
    #[serde(skip_serializing_if = "is_false")]
    deity: bool,
    /// Other markers, worded like `match_replay log` (`Mag:2`, `Gems:3`, `locked:1t`, ...).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    notes: Vec<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// What the actor noted about an action ([`pb::Note`]).
#[derive(Serialize)]
struct NoteView {
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<f32>,
    /// The probability of the action taken: `exp(logp)`.
    #[serde(skip_serializing_if = "Option::is_none")]
    p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    logp: Option<f32>,
    #[serde(skip_serializing_if = "is_false")]
    forced: bool,
    /// The candidate actions, most probable first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    candidates: Vec<CandidateView>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    extra: BTreeMap<String, f64>,
}

#[derive(Serialize)]
struct CandidateView {
    action: String,
    prob: f32,
    /// The action taken.
    #[serde(skip_serializing_if = "is_false")]
    chosen: bool,
}

/// A combat between players `a` (side A) and `b` (side B).
#[derive(Serialize)]
struct CombatView {
    a: usize,
    b: usize,
    seed: String,
    /// `a`, `b` or `draw`.
    outcome: &'static str,
    damage: u32,
    /// The units each side fought with: their boards in the previous frame, at most 7 each. Side
    /// A's units have combat ids `0..lenA`, side B's the ids after them.
    boards: [Vec<usize>; 2],
    survivors: [Vec<usize>; 2],
    /// Every event, with units named like `match_replay log` names them ([`CombatNames`]).
    events: Vec<EventView>,
}

/// `[kind, side, text]`: the event's kind (`attack`, `death`, ...), the side it is about (`0` for
/// A, `1` for B, `null` for neither) and its text without its number.
#[derive(Serialize)]
struct EventView(&'static str, Option<u8>, String);

impl<'a> ViewerData<'a> {
    fn new(timeline: &'a Timeline, source: &'a str) -> Self {
        let header = &timeline.header;
        let names: Vec<String> = (0..timeline.num_players())
            .map(|p| player_name(timeline, p))
            .collect();
        let shared = header.pool() == pb::PoolMode::Shared;
        let setup = SetupView {
            engine: &header.engine,
            format: header.format,
            catalog: &header.catalog,
            pool: if shared { "shared" } else { "per-player" },
            shared_seed: shared.then(|| header.shared_seed.to_string()),
            players: header
                .players
                .iter()
                .zip(&names)
                .map(|(setup, name)| PlayerSetupView {
                    name: name.clone(),
                    deity: format_deity_kind(replay::deity_from_pb(setup.deity())),
                    armor: setup.armor,
                    shop_spells: setup.shop_spells,
                    seed: (!shared).then(|| setup.seed.to_string()),
                })
                .collect(),
            meta: &header.meta,
        };
        let mut frames = Frames {
            timeline,
            names,
            units: Units::default(),
            shown_states: vec![None; timeline.num_players()],
            shown_pools: vec![None; timeline.num_players()],
        };
        let frame_views = (0..timeline.len()).map(|i| frames.frame(i)).collect();
        let result = match timeline.frames.last().map(|f| &f.kind) {
            Some(FrameKind::End { .. }) => Some(frame_title(timeline, timeline.len() - 1)),
            _ => None,
        };
        Self {
            source,
            setup,
            viewer: replay::engine_version(),
            engine_mismatch: timeline.engine_mismatch(),
            faithful: timeline.is_faithful(),
            divergences: timeline
                .divergences
                .iter()
                .map(|d| DivergenceView {
                    record: d.record,
                    frame: d.frame,
                    message: &d.message,
                })
                .collect(),
            error: timeline.error.as_ref().map(|e| ErrorView {
                record: e.record,
                message: &e.message,
            }),
            result,
            actions: timeline.decisions().count(),
            turns: timeline.last_turn(),
            units: frames.units.list,
            frames: frame_views,
        }
    }
}

/// Builds the frame views in order, remembering what earlier frames already show.
struct Frames<'t> {
    timeline: &'t Timeline,
    names: Vec<String>,
    units: Units,
    /// The last state shown of each player.
    shown_states: Vec<Option<PlayerView>>,
    shown_pools: Vec<Option<PoolView>>,
}

impl Frames<'_> {
    fn frame(&mut self, i: usize) -> FrameView {
        let timeline = self.timeline;
        let frame = &timeline.frames[i];
        let prev = i.checked_sub(1).map(|j| &timeline.frames[j]);
        let mut states = Vec::new();
        let mut pools = Vec::new();
        let mut auras = Vec::new();
        for (p, state) in frame.players.iter().enumerate() {
            let view = PlayerView::new(state, &mut self.units);
            states.push(changed(&mut self.shown_states[p], view));
            let pool = PoolView::new(state, &frame_pool(timeline, frame, p));
            pools.push(changed(&mut self.shown_pools[p], pool));
            if let Some(prev) = prev {
                let diffs = diff_auras(&prev.players[p].auras, &state.auras);
                if !diffs.is_empty() {
                    auras.push((p, diffs));
                }
            }
        }

        let mut title = frame_title(timeline, i);
        let mut view = FrameView {
            kind: "start",
            turn: frame.turn,
            record: frame.record,
            title: String::new(),
            label: None,
            actors: Vec::new(),
            states,
            pools,
            auras,
            note: None,
            combat: None,
            winner: None,
            divergences: timeline
                .divergences
                .iter()
                .filter(|d| d.frame == i)
                .map(|d| d.message.clone())
                .collect(),
        };
        let before = |p: usize| &prev.expect("only the first frame has no call").players[p];
        match &frame.kind {
            FrameKind::Start => {}
            FrameKind::StartTurn { player, .. } => {
                view.kind = "turn";
                view.actors = vec![*player];
            }
            FrameKind::Act {
                player,
                action,
                note,
                ..
            } => {
                let state = before(*player);
                view.kind = "act";
                view.actors = vec![*player];
                view.label = Some(format!(
                    "{}: {}",
                    self.names[*player],
                    short_action(state, action)
                ));
                if let Some(note) = note {
                    // The page shows the note on its own.
                    let suffix = note_text(Some(note));
                    if let Some(stripped) = title.strip_suffix(suffix.as_str()) {
                        title.truncate(stripped.len());
                    }
                    view.note = Some(NoteView::new(note, state, action));
                }
            }
            FrameKind::Combat { a, b, seed, result } => {
                view.kind = "combat";
                view.actors = vec![*a, *b];
                view.combat = Some(self.combat([*a, *b], *seed, result, [before(*a), before(*b)]));
            }
            FrameKind::SetPairs { .. } | FrameKind::RecordDeaths { .. } => view.kind = "lobby",
            FrameKind::DeliverGifts => {
                view.kind = "lobby";
                let before = prev.expect("only the first frame has no call");
                view.actors = (0..frame.players.len())
                    .filter(|&p| before.players[p] != frame.players[p])
                    .collect();
            }
            FrameKind::End { winner } => {
                view.kind = "end";
                view.winner = *winner;
            }
        }
        view.title = title;
        view
    }

    fn combat(
        &mut self,
        [a, b]: [usize; 2],
        seed: u64,
        result: &BattleResult,
        [pre_a, pre_b]: [&TavernState; 2],
    ) -> CombatView {
        let mut names = CombatNames::new([&self.names[a], &self.names[b]], pre_a, pre_b);
        let events = result
            .events
            .iter()
            .map(|ev| {
                let text = names.describe(ev);
                let side = event_side(&names, ev).map(|side| match side {
                    Side::A => 0,
                    Side::B => 1,
                });
                EventView(event_kind(ev), side, text)
            })
            .collect();
        CombatView {
            a,
            b,
            seed: seed.to_string(),
            outcome: match result.outcome {
                BattleOutcome::AWin => "a",
                BattleOutcome::BWin => "b",
                BattleOutcome::Draw => "draw",
            },
            damage: result.hero_damage,
            boards: [
                self.units.ids(fighters(pre_a)),
                self.units.ids(fighters(pre_b)),
            ],
            survivors: [
                self.units.ids(&result.survivors_a),
                self.units.ids(&result.survivors_b),
            ],
            events,
        }
    }
}

/// The units of a player's board that fight in a combat.
fn fighters(state: &TavernState) -> &[Unit] {
    &state.board[..state.board.len().min(MAX_BOARD_SIZE)]
}

/// `Some(new)` if it differs from what `shown` holds (which then holds it), else `None`.
fn changed<T: Clone + PartialEq>(shown: &mut Option<T>, new: T) -> Option<T> {
    if shown.as_ref() == Some(&new) {
        return None;
    }
    *shown = Some(new.clone());
    Some(new)
}

/// Interns units by what the page shows of them.
#[derive(Default)]
struct Units {
    index: HashMap<UnitView, usize>,
    list: Vec<UnitView>,
}

impl Units {
    fn id(&mut self, unit: &Unit) -> usize {
        let view = UnitView::new(unit);
        if let Some(&id) = self.index.get(&view) {
            return id;
        }
        let id = self.list.len();
        self.index.insert(view.clone(), id);
        self.list.push(view);
        id
    }

    fn ids(&mut self, units: &[Unit]) -> Vec<usize> {
        units.iter().map(|u| self.id(u)).collect()
    }
}

impl PlayerView {
    fn new(state: &TavernState, units: &mut Units) -> Self {
        let auras = &state.auras;
        Self {
            turn: state.turn,
            health: state.health,
            armor: state.armor,
            tier: state.tavern_tier,
            gold: state.gold,
            max_gold: state.max_gold,
            bonus_gold: state.bonus_gold_next_turn,
            // `match_replay log` shows MAX_TIER from Tier 6.
            upgrade: (state.tavern_tier < 6).then_some(state.upgrade_cost),
            frozen: state.is_frozen,
            deity: DeityView {
                kind: format_deity_kind(auras.deity.kind),
                attack: auras.deity.attack,
                health: auras.deity.health,
                golden: auras.deity.is_golden,
            },
            spells_cast: auras.tavern_spells_cast,
            spell_buff: [auras.spell_bonus_atk, auras.spell_bonus_hp],
            blood_gem: [1 + auras.blood_gem_bonus_atk, 1 + auras.blood_gem_bonus_hp],
            undead_attack: auras.undead_bonus_attack,
            free_refreshes: auras.free_refreshes,
            board: units.ids(&state.board),
            hand: units.ids(&state.hand),
            shop: units.ids(&state.shop),
            discover: state.discover_pending.as_deref().map(|o| units.ids(o)),
            queued: state.discover_queue.len(),
        }
    }
}

impl PoolView {
    fn new(state: &TavernState, pool: &dyn PoolCounts) -> Self {
        let counts =
            |units: &[Unit]| -> Vec<_> { units.iter().map(|u| pool_copies(u, pool)).collect() };
        Self {
            shop: counts(&state.shop),
            discover: state.discover_pending.as_deref().map(counts),
        }
    }
}

/// `[copies left, base copies]` of a minion drawn from the pool, for the units
/// `match_replay log` shows them for.
fn pool_copies(unit: &Unit, pool: &dyn PoolCounts) -> Option<[u32; 2]> {
    let drawn =
        !unit.is_spell && (100..800).contains(&unit.card_id) && (1..=7).contains(&unit.tavern_tier);
    drawn.then(|| {
        [
            pool.copies_left(unit.card_id),
            base_copies_for_tier(unit.tavern_tier),
        ]
    })
}

impl UnitView {
    fn new(u: &Unit) -> Self {
        let keywords = [
            (u.taunt, "Taunt"),
            (u.divine_shield, "Divine Shield"),
            (u.windfury, "Windfury"),
            (u.reborn, "Reborn"),
            (u.venomous, "Venomous"),
            (u.stealth, "Stealth"),
            (u.magnetic, "Magnetic"),
        ]
        .into_iter()
        .filter_map(|(has, keyword)| has.then_some(keyword))
        .collect();
        // The markers `format_unit_inline` shows.
        let mut notes = Vec::new();
        if u.is_spell {
            if u.lockbox_turns_left > 0 {
                notes.push(format!("opens in {}t", u.lockbox_turns_left));
            }
            if u.locked_turns > 0 {
                notes.push(format!("locked {}t", u.locked_turns));
            }
            if u.combine_choose_one {
                notes.push("FandralCombined".to_string());
            }
        } else {
            if u.magnetizations_count > 0 {
                notes.push(format!("Mag:{}", u.magnetizations_count));
            }
            if u.blood_gems_played > 0 {
                notes.push(format!("Gems:{}", u.blood_gems_played));
            }
            if u.eot_health_bonus > 0 {
                notes.push(format!("+{}HP/eot", u.eot_health_bonus));
            }
            if u.sot_gold_bonus > 0 {
                notes.push(format!("+{}g/sot", u.sot_gold_bonus));
            }
            if u.locked_turns > 0 {
                notes.push(format!("locked:{}t", u.locked_turns));
            }
            if u.dies_on_play_this_turn {
                notes.push("DiesOnPlay".to_string());
            }
        }
        let activate = if u.is_spell {
            None
        } else {
            cards::activate_cost(u.card_id)
        };
        Self {
            name: u.name.clone(),
            card: u.card_id,
            tier: u.tavern_tier,
            attack: u.attack,
            health: u.health,
            tribe: (!u.is_spell && u.tribe != Tribe::None).then(|| format_tribe(u.tribe)),
            keywords,
            golden: u.is_golden,
            spell: u.is_spell.then(|| {
                let currency = if u.costs_health { " HP" } else { "g" };
                format!("{}{currency}", u.spell_cost)
            }),
            activate,
            activated: activate.is_some() && u.activated_this_turn,
            deity: u.is_deity,
            notes,
        }
    }
}

impl NoteView {
    /// The note of `action`, taken in `state`.
    fn new(note: &pb::Note, state: &TavernState, action: &TavernAction) -> Self {
        let mut candidates: Vec<CandidateView> = note
            .candidates
            .iter()
            .map(|candidate| {
                let decoded = candidate
                    .action
                    .as_ref()
                    .and_then(|a| replay::action_from_pb(a).ok());
                CandidateView {
                    action: decoded.map_or_else(|| "?".to_string(), |a| short_action(state, &a)),
                    // Four decimals are plenty for a bar and a percentage.
                    prob: (candidate.prob * 1e4).round() / 1e4,
                    chosen: decoded.as_ref() == Some(action),
                }
            })
            .collect();
        candidates.sort_by(|x, y| y.prob.total_cmp(&x.prob));
        Self {
            value: note.value,
            p: note.logp.map(f32::exp),
            logp: note.logp,
            forced: note.forced,
            candidates,
            extra: note.extra.clone(),
        }
    }
}

/// A short description of `action` in `state` (`Buy Joyous`, `Cast Fortify on Zoatroid`), for
/// the frame list and note candidates. [`frame_title`] has the full one.
fn short_action(state: &TavernState, action: &TavernAction) -> String {
    let name = |units: &[Unit], i: usize| {
        units
            .get(i)
            .map_or_else(|| format!("#{i}"), |u| u.name.clone())
    };
    match *action {
        TavernAction::Buy { shop_index } => format!("Buy {}", name(&state.shop, shop_index)),
        TavernAction::Play {
            hand_index,
            board_pos,
        } => match state.hand.get(hand_index) {
            Some(card) if card.is_spell => {
                if spells::spell_requires_board_target(card.card_id) {
                    format!("Cast {} on {}", card.name, name(&state.board, board_pos))
                } else {
                    format!("Cast {}", card.name)
                }
            }
            Some(card)
                if card.magnetic
                    && state
                        .board
                        .get(board_pos)
                        .is_some_and(|target| target.tribe.matches(card.tribe)) =>
            {
                format!(
                    "Magnetize {} onto {}",
                    card.name, state.board[board_pos].name
                )
            }
            Some(card) => format!("Play {} at {board_pos}", card.name),
            None => format!("Play hand[{hand_index}] at {board_pos}"),
        },
        TavernAction::Sell { board_pos } => format!("Sell {}", name(&state.board, board_pos)),
        TavernAction::Reposition { from_pos, to_pos } => format!(
            "Move {} from {from_pos} to {to_pos}",
            name(&state.board, from_pos)
        ),
        TavernAction::Activate {
            board_pos,
            target_pos,
        } => {
            let source = name(&state.board, board_pos);
            match target_pos {
                None => format!("Activate {source}"),
                Some(t) => {
                    let kind = state
                        .board
                        .get(board_pos)
                        .map(|u| cards::activate_target_kind(u.card_id));
                    let zone = match kind {
                        Some(ActivateTargetKind::HandCard) => &state.hand,
                        Some(ActivateTargetKind::ShopCard) => &state.shop,
                        _ => &state.board,
                    };
                    format!("Activate {source} on {}", name(zone, t))
                }
            }
        }
        TavernAction::Refresh => "Refresh".to_string(),
        TavernAction::UpgradeTavern => format!("Upgrade to Tier {}", state.tavern_tier + 1),
        TavernAction::ToggleFreeze => if state.is_frozen {
            "Unfreeze"
        } else {
            "Freeze"
        }
        .to_string(),
        TavernAction::ChooseDiscover { option_index } => match &state.discover_pending {
            Some(options) => format!("Choose {}", name(options, option_index)),
            None => format!("Choose option {option_index}"),
        },
        TavernAction::EndTurn => "End turn".to_string(),
        TavernAction::PlayOnShop {
            hand_index,
            shop_index,
        } => format!(
            "Cast {} on {}",
            name(&state.hand, hand_index),
            name(&state.shop, shop_index)
        ),
        TavernAction::Pass { hand_index } => format!("Pass {}", name(&state.hand, hand_index)),
        TavernAction::HeroPower { target } | TavernAction::HeroPower2 { target } => {
            let power = state
                .hero()
                .and_then(|h| heroes::data(h.hero_id))
                .map_or("Hero Power", |h| h.power.name);
            match target {
                Some(t) => format!("{power} on {}", name(&state.board, t)),
                None => power.to_string(),
            }
        }
        TavernAction::ChooseHero { option_index } => {
            let hero = state
                .hero_options
                .get(option_index)
                .and_then(|&id| heroes::data(id))
                .map_or("?", |h| h.name);
            format!("Choose hero {hero}")
        }
        TavernAction::ChooseTrinket { option_index } => {
            match state.trinket_options.get(option_index) {
                Some(o) => {
                    let name = trinkets::data(o.card_id).map_or("?", |t| t.name);
                    format!("Choose trinket {name}")
                }
                None => format!("Choose trinket {option_index}"),
            }
        }
        TavernAction::DarkDiscovery => "Dark Discovery".to_string(),
    }
}

fn event_kind(ev: &Event) -> &'static str {
    match ev {
        Event::BattleStart { .. } => "start",
        Event::AttackDeclared { .. } => "attack",
        Event::StatBuff { .. } => "buff",
        Event::UnitSummoned { .. } => "summon",
        Event::DamageDealt { .. } => "damage",
        Event::DivineShieldPopped { .. } => "shield",
        Event::VenomousTriggered { .. } => "venom",
        Event::Death { .. } => "death",
        Event::DeityAwakened { .. } => "deity",
        Event::UnitTransformed { .. } => "summon",
        Event::WarbandSteppedIn { .. } => "start",
        Event::TurnSkipped { .. } => "skip",
        Event::BattleEnd { .. } => "end",
    }
}

/// The side an event is about: the side acting, or the side of the unit it happens to.
fn event_side(names: &CombatNames, ev: &Event) -> Option<Side> {
    let side_of = |uid| names.unit(uid).map(|(side, _)| side);
    match ev {
        Event::BattleStart { first_attacker, .. } => Some(*first_attacker),
        Event::AttackDeclared { side, .. }
        | Event::StatBuff { side, .. }
        | Event::UnitSummoned { side, .. }
        | Event::DeityAwakened { side, .. }
        | Event::UnitTransformed { side, .. }
        | Event::TurnSkipped { side } => Some(*side),
        Event::WarbandSteppedIn { team, .. } => Some(*team),
        Event::DamageDealt { unit, .. }
        | Event::DivineShieldPopped { unit }
        | Event::Death { unit } => side_of(*unit),
        Event::VenomousTriggered { attacker, .. } => side_of(*attacker),
        Event::BattleEnd { outcome, .. } => match outcome {
            BattleOutcome::AWin => Some(Side::A),
            BattleOutcome::BWin => Some(Side::B),
            BattleOutcome::Draw => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use seaglass::{DeityKind, Match, MatchSetup, PlayerSetup, PoolMode, Rng};
    use seaglass_tools::bot::choose_semi_random_action;
    use serde_json::Value;

    /// A player name that would end the data's <script> element if it were not escaped.
    const SNEAKY: &str = "</script><!-- P2";

    /// A whole match between `sim_match`'s bots, with a note on every action.
    fn recorded_match(seed: u64) -> pb::Replay {
        let player = |name: &str, deity| PlayerSetup {
            name: name.to_string(),
            deity,
            ..Default::default()
        };
        let mut game = Match::new(&MatchSetup {
            pool: PoolMode::Shared,
            shared_seed: seed,
            players: vec![
                player("P1", DeityKind::CThun),
                player(SNEAKY, DeityKind::YShaarj),
            ],
            meta: [("source".to_string(), "html test".to_string())].into(),
            ..Default::default()
        })
        .unwrap();
        let mut bot = Rng::new(seed ^ 0x5EED);
        for _turn in 0..30 {
            game.start_turn(0);
            game.start_turn(1);
            for p in 0..2 {
                for step in 0..40 {
                    let state = game.player(p);
                    let forced = step == 39 && state.discover_pending.is_none();
                    let action = if forced {
                        TavernAction::EndTurn
                    } else {
                        choose_semi_random_action(state, &mut bot)
                    };
                    let legal = state.valid_actions();
                    let prob = 1.0 / legal.len().max(1) as f32;
                    let note = pb::Note {
                        value: Some(0.1 + p as f32 / 4.0),
                        logp: Some(prob.ln()),
                        forced,
                        candidates: legal
                            .iter()
                            .map(|a| pb::Candidate {
                                action: Some(replay::action_to_pb(a)),
                                prob,
                            })
                            .collect(),
                        extra: [("step".to_string(), step as f64)].into(),
                    };
                    match game.step_with_note(p, action, Some(note)) {
                        Ok(false) => {}
                        Ok(true) | Err(_) => break,
                    }
                }
            }
            game.combat(0, 1);
            if game.players().iter().any(|s| s.health <= 0) {
                break;
            }
        }
        game.finish();
        game.replay()
    }

    /// The JSON embedded in a page.
    fn embedded_data(page: &str) -> Value {
        let open = r#"<script type="application/json" id="replay-data">"#;
        let start = page.find(open).expect("the page has the data element") + open.len();
        let len = page[start..]
            .find("</script>")
            .expect("the data element ends");
        let json = &page[start..start + len];
        assert!(!json.contains('<'), "the data must not contain `<`");
        serde_json::from_str(json).expect("the embedded data is JSON")
    }

    fn names<'v>(units: &'v [Value], ids: &Value) -> Vec<&'v str> {
        ids.as_array()
            .unwrap()
            .iter()
            .map(|id| {
                units[id.as_u64().unwrap() as usize]["name"]
                    .as_str()
                    .unwrap()
            })
            .collect()
    }

    fn unit_names(units: &[Unit]) -> Vec<&str> {
        units.iter().map(|u| u.name.as_str()).collect()
    }

    #[test]
    fn the_page_embeds_every_frame() {
        let timeline = Timeline::new(&recorded_match(7)).unwrap();
        assert!(timeline.is_faithful());
        let page = render(&timeline, "test.replay.pb");
        assert!(!page.contains(DATA));
        let data = embedded_data(&page);

        assert_eq!(data["source"], "test.replay.pb");
        assert_eq!(data["faithful"], true);
        assert_eq!(data["setup"]["players"][1]["name"], SNEAKY);
        assert_eq!(data["setup"]["meta"]["source"], "html test");
        assert_eq!(
            data["result"].as_str(),
            Some(frame_title(&timeline, timeline.len() - 1).as_str())
        );

        let units = data["units"].as_array().unwrap();
        let frames = data["frames"].as_array().unwrap();
        assert_eq!(frames.len(), timeline.len());
        let mut shown: Vec<Value> = vec![Value::Null; timeline.num_players()];
        let (mut combats, mut notes) = (0, 0);
        for (i, (frame, json)) in timeline.frames.iter().zip(frames).enumerate() {
            assert_eq!(json["turn"], frame.turn, "frame {i}");
            // Every player's state, carried forward from the frame that last changed it.
            for (p, state) in frame.players.iter().enumerate() {
                let view = &json["states"][p];
                assert!(i > 0 || !view.is_null(), "frame 0 shows every player");
                if !view.is_null() {
                    shown[p] = view.clone();
                }
                let s = &shown[p];
                assert_eq!(s["health"], state.health, "frame {i}, player {p}");
                assert_eq!(s["gold"], state.gold, "frame {i}, player {p}");
                assert_eq!(names(units, &s["board"]), unit_names(&state.board));
                assert_eq!(names(units, &s["hand"]), unit_names(&state.hand));
                assert_eq!(names(units, &s["shop"]), unit_names(&state.shop));
            }
            let kind = match &frame.kind {
                FrameKind::Start => "start",
                FrameKind::StartTurn { .. } => "turn",
                FrameKind::SetPairs { .. }
                | FrameKind::DeliverGifts
                | FrameKind::RecordDeaths { .. } => "lobby",
                FrameKind::Act { note, .. } => {
                    let note = note.as_ref().expect("every action has a note");
                    let candidates = json["note"]["candidates"].as_array().unwrap();
                    assert_eq!(candidates.len(), note.candidates.len(), "frame {i}");
                    assert_eq!(
                        candidates.iter().filter(|c| c["chosen"] == true).count(),
                        1,
                        "frame {i}: the action taken is a candidate"
                    );
                    assert_eq!(
                        format!(
                            "{}{}",
                            json["title"].as_str().unwrap(),
                            note_text(Some(note))
                        ),
                        frame_title(&timeline, i)
                    );
                    notes += 1;
                    "act"
                }
                FrameKind::Combat { a, b, result, .. } => {
                    let combat = &json["combat"];
                    assert_eq!(combat["damage"], result.hero_damage);
                    let events = combat["events"].as_array().unwrap();
                    assert_eq!(events.len(), result.events.len(), "frame {i}");
                    let prev = &timeline.frames[i - 1];
                    for (side, p) in [*a, *b].into_iter().enumerate() {
                        let board = &prev.players[p].board;
                        let fighters = &board[..board.len().min(MAX_BOARD_SIZE)];
                        assert_eq!(names(units, &combat["boards"][side]), unit_names(fighters));
                    }
                    // Units are named like `match_replay log` names them.
                    let labels = [player_name(&timeline, *a), player_name(&timeline, *b)];
                    let mut combat_names = CombatNames::new(
                        [&labels[0], &labels[1]],
                        &prev.players[*a],
                        &prev.players[*b],
                    );
                    for (event, json) in result.events.iter().zip(events) {
                        assert_eq!(json[2], combat_names.describe(event));
                    }
                    combats += 1;
                    "combat"
                }
                FrameKind::End { .. } => "end",
            };
            assert_eq!(json["kind"], kind, "frame {i}");
        }
        assert!(combats > 0 && notes > 0);
    }

    #[test]
    fn the_template_has_one_data_placeholder() {
        assert_eq!(TEMPLATE.matches(DATA).count(), 1);
        assert!(TEMPLATE.contains(&format!(
            r#"<script type="application/json" id="replay-data">{DATA}</script>"#
        )));
    }

    #[test]
    fn the_page_goes_next_to_the_replay() {
        let out = |input: &str| default_output(Path::new(input));
        assert_eq!(out("replays/s7.replay.pb"), Path::new("replays/s7.html"));
        assert_eq!(out("game.pb"), Path::new("game.html"));
        assert_eq!(out("game"), Path::new("game.html"));
        assert_eq!(out("runs/v1.2/game"), Path::new("runs/v1.2/game.html"));
        assert_eq!(out(".replay.pb"), Path::new(".replay.html"));
    }
}
