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
    base_copies_for_tier, ActivateTargetKind, BattleOutcome, BattleResult, CardId, CardPool, Event,
    HeroId, HeroPowerState, Rng, Side, TavernAction, TavernState, Timeline, Tribe, Unit,
};
use seaglass::{dark_gifts, heroes, trinkets};
use seaglass_tools::fmt::{diff_auras, format_deity_kind, format_tribe, CombatNames, PoolCounts};
use seaglass_tools::render::text::card_text;
use seaglass_tools::replay_text::{frame_pool, frame_title, note_text, player_name, unit_players};

/// The viewer page. [`DATA`] marks where the replay's JSON goes.
const TEMPLATE: &str = include_str!("../../../viewer/replay.html");
const DATA: &str = "__REPLAY_DATA__";
/// The one-player page (`match_replay html --player P`), with the same [`DATA`] placeholder.
const PLAYER_TEMPLATE: &str = include_str!("../../../viewer/player.html");

/// A combat uses at most this many units of each board.
const MAX_BOARD_SIZE: usize = 7;

/// `match_replay html <file> [--player P] [-o out.html]`.
pub fn run(args: &[String]) -> Result<ExitCode, String> {
    let timeline = crate::load_one(args)?;
    let input = Path::new(&args[0]);
    let focus = match crate::option(args, &["--player"]) {
        Some(text) => Some(crate::parse_player(&timeline, text)?),
        None if args.iter().any(|a| a == "--player") => {
            return Err("--player needs a player (a name or a 0-based index)".to_string());
        }
        None => None,
    };
    let output = match crate::option(args, &["-o", "--output"]) {
        Some(path) => PathBuf::from(path),
        None if args.iter().any(|a| a == "-o" || a == "--output") => {
            return Err("-o needs an output file".to_string());
        }
        None => match focus {
            Some(p) => default_player_output(input, p),
            None => default_output(input),
        },
    };
    let source = input.file_name().map_or_else(
        || args[0].clone(),
        |name| name.to_string_lossy().into_owned(),
    );
    let page = match focus {
        Some(p) => render_player(&timeline, &source, p),
        None => render(&timeline, &source),
    };
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
    with_suffix(input, "html")
}

/// The default path of the page that follows player `player`: like [`default_output`], ending in
/// `.p<player>.html`.
fn default_player_output(input: &Path, player: usize) -> PathBuf {
    with_suffix(input, &format!("p{player}.html"))
}

/// `input` with `.replay.pb` (or else its extension) replaced by `.suffix`.
fn with_suffix(input: &Path, suffix: &str) -> PathBuf {
    let name = input.file_name().map(|n| n.to_string_lossy());
    match name.as_deref().and_then(|n| n.strip_suffix(".replay.pb")) {
        Some(stem) if !stem.is_empty() => input.with_file_name(format!("{stem}.{suffix}")),
        _ => input.with_extension(suffix),
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

/// The one-player page of `timeline`, starting on player `focus`. Its data is [`render`]'s, plus
/// [`ViewerData::focus`].
pub fn render_player(timeline: &Timeline, source: &str, focus: usize) -> String {
    let mut data = ViewerData::new(timeline, source);
    data.focus = Some(focus);
    let json = serde_json::to_string(&data).expect("the viewer data is plain JSON");
    PLAYER_TEMPLATE.replacen(DATA, &json.replace('<', "\\u003c"), 1)
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
    /// The rules text of the units' cards, by card id (cards whose source documents one;
    /// [`card_text`]).
    card_text: BTreeMap<u32, &'static str>,
    /// The rules text of the trinkets, Hero Powers and heroes the player states name, keyed
    /// `trinket:<name>`, `power:<name>` and `hero:<name>` (a hero's text is its Hero Power's).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    texts: BTreeMap<String, String>,
    /// One per timeline frame.
    frames: Vec<FrameView>,
    /// The player the one-player page starts on (`--player`). Not in the normal page's data.
    #[serde(skip_serializing_if = "Option::is_none")]
    focus: Option<usize>,
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
    /// The action of an `act` frame, structured for drawing ([`ActionView`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<ActionView>,
    /// The coming turn's pairings of a `SetPairs` frame (units; players in Solo).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pairs: Vec<[usize; 2]>,
    /// The result of a ghost combat, from the fighting unit's side.
    #[serde(skip_serializing_if = "Option::is_none")]
    ghost: Option<GhostView>,
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
    /// The hero and its Hero Powers (`null` before a hero is chosen, and without heroes).
    hero: Option<HeroView>,
    /// What a refresh costs now (the page shows 0 while `free_refreshes` is above 0).
    refresh_cost: u32,
    /// The names of the trinkets the player owns.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    trinkets: Vec<String>,
    /// The names of the heroes on offer.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    hero_options: Vec<String>,
    /// The trinkets on offer.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    trinket_options: Vec<TrinketOptionView>,
    /// `[uses left, uses per game]` of the Dark Discovery button, if the game has Dark Gifts.
    #[serde(skip_serializing_if = "Option::is_none")]
    dark_discovery: Option<[u32; 2]>,
}

/// A player's hero.
#[derive(Serialize, Clone, PartialEq)]
struct HeroView {
    name: String,
    /// The Hero Powers: the first, then the second one if the hero has one.
    powers: Vec<PowerView>,
}

#[derive(Serialize, Clone, PartialEq)]
struct PowerView {
    name: String,
    /// Its current Gold cost.
    cost: u32,
    /// It has no uses left this turn.
    used: bool,
}

/// A trinket on offer.
#[derive(Serialize, Clone, PartialEq)]
struct TrinketOptionView {
    name: String,
    /// Its price in Gold.
    cost: u32,
}

/// The result of a ghost combat from the point of view of the unit that fought the ghost.
#[derive(Serialize)]
struct GhostView {
    /// The unit that fought.
    unit: usize,
    /// The unit whose warband the ghost copies.
    ghost: usize,
    /// `win`, `loss` or `draw`.
    outcome: &'static str,
    /// The damage the loser takes (the combat's `hero_damage`).
    damage: u32,
}

/// An action, structured so the page can draw it. Indices are the [`TavernAction`]'s, into the
/// actor's state before the action. Optional fields are always present (`null` when absent).
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ActionView {
    Buy {
        shop: usize,
    },
    /// A minion played to board index `board`.
    Play {
        hand: usize,
        board: usize,
    },
    /// A Magnetic minion played onto the board minion `board`.
    Magnetize {
        hand: usize,
        board: usize,
    },
    /// A spell; `target` is a board index if the spell needs one.
    Cast {
        hand: usize,
        target: Option<usize>,
    },
    CastOnShop {
        hand: usize,
        shop: usize,
    },
    Sell {
        board: usize,
    },
    /// The minion at `from` ends up at index `to`.
    Move {
        from: usize,
        to: usize,
    },
    /// `zone` (`board`, `hand` or `shop`) is where `target` is; both `null` without a target.
    Activate {
        board: usize,
        target: Option<usize>,
        zone: Option<&'static str>,
    },
    Refresh,
    Upgrade,
    /// `frozen` is the shop's state after the action.
    Freeze {
        frozen: bool,
    },
    Discover {
        option: usize,
    },
    EndTurn,
    Pass {
        hand: usize,
    },
    /// `second` is the second Hero Power.
    HeroPower {
        target: Option<usize>,
        second: bool,
    },
    ChooseHero {
        option: usize,
    },
    ChooseTrinket {
        option: usize,
    },
    DarkDiscovery,
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
    /// The candidate actions: best score first when they have scores, else most probable first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    candidates: Vec<CandidateView>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    extra: BTreeMap<String, f64>,
}

#[derive(Serialize)]
struct CandidateView {
    action: String,
    /// The probability the actor gave the action (a policy).
    #[serde(skip_serializing_if = "Option::is_none")]
    prob: Option<f32>,
    /// The actor's score for its best line starting with `action` (a search).
    #[serde(skip_serializing_if = "Option::is_none")]
    score: Option<f32>,
    /// What kind of line it is (`roll`, `dark discovery`, ...).
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    /// The rest of the line after `action`, one short description per step.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    plan: Vec<String>,
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
            card_text: frames
                .units
                .list
                .iter()
                .filter_map(|u| card_text(u.card).map(|text| (u.card, text)))
                .collect(),
            texts: frames.units.texts,
            units: frames.units.list,
            frames: frame_views,
            focus: None,
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
            action: None,
            pairs: Vec::new(),
            ghost: None,
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
                view.actors = unit_players(timeline, *player);
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
                view.action = Some(ActionView::new(state, action));
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
            FrameKind::SetPairs { pairs } => {
                view.kind = "lobby";
                view.pairs = pairs.iter().map(|&(a, b)| [a, b]).collect();
            }
            FrameKind::RecordDeaths { .. } => view.kind = "lobby",
            FrameKind::DeliverGifts => {
                view.kind = "lobby";
                let before = prev.expect("only the first frame has no call");
                view.actors = (0..frame.players.len())
                    .filter(|&p| before.players[p] != frame.players[p])
                    .collect();
            }
            FrameKind::DuosCombat { a, b, .. } => {
                // Duos combats are shown by their title for now (no event playback).
                view.kind = "lobby";
                view.actors = [*a, *b]
                    .iter()
                    .flat_map(|&u| unit_players(timeline, u))
                    .collect();
            }
            FrameKind::GhostCombat {
                unit,
                ghost,
                result,
                ..
            } => {
                view.kind = "lobby";
                view.actors = unit_players(timeline, *unit);
                view.ghost = Some(GhostView {
                    unit: *unit,
                    ghost: *ghost,
                    outcome: match result.outcome() {
                        BattleOutcome::AWin => "win",
                        BattleOutcome::BWin => "loss",
                        BattleOutcome::Draw => "draw",
                    },
                    damage: result.hero_damage(),
                });
            }
            FrameKind::End { winner, .. } => {
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

/// Interns units by what the page shows of them, and collects the rules text of the trinkets,
/// Hero Powers and heroes the player views name ([`ViewerData::texts`]).
#[derive(Default)]
struct Units {
    index: HashMap<UnitView, usize>,
    list: Vec<UnitView>,
    texts: BTreeMap<String, String>,
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

    /// The name of trinket `card_id`, noting its text.
    fn trinket(&mut self, card_id: CardId) -> String {
        let Some(data) = trinkets::data(card_id) else {
            return "?".to_string();
        };
        self.texts
            .entry(format!("trinket:{}", data.name))
            .or_insert_with(|| data.text.to_string());
        data.name.to_string()
    }

    /// The name of hero `id`, noting its text and its Hero Power's.
    fn hero(&mut self, id: HeroId) -> String {
        let Some(data) = heroes::data(id) else {
            return "?".to_string();
        };
        let power = &data.power;
        self.texts
            .entry(format!("power:{}", power.name))
            .or_insert_with(|| power.text.to_string());
        self.texts
            .entry(format!("hero:{}", data.name))
            .or_insert_with(|| {
                format!(
                    "Hero Power: {} ({} Gold). {}",
                    power.name, power.cost, power.text
                )
            });
        data.name.to_string()
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
            hero: state.hero().map(|hero| {
                let name = |card_id| heroes::data(card_id).map_or("Hero Power", |h| h.power.name);
                let power = |name: &str, p: &HeroPowerState| PowerView {
                    name: name.to_string(),
                    cost: p.cost,
                    used: p.uses_left == 0,
                };
                // The engine has no name for a power's `CardId` (only a hero's first power).
                let mut powers = vec![power(name(hero.hero_id), &hero.power)];
                powers.extend(hero.second_power.as_ref().map(|p| power("Hero Power 2", p)));
                HeroView {
                    name: units.hero(hero.hero_id),
                    powers,
                }
            }),
            refresh_cost: state.refresh_cost(),
            trinkets: auras
                .trinkets
                .iter()
                .map(|t| units.trinket(t.card_id))
                .collect(),
            hero_options: state
                .hero_options
                .iter()
                .map(|&id| units.hero(id))
                .collect(),
            trinket_options: state
                .trinket_options
                .iter()
                .map(|o| TrinketOptionView {
                    name: units.trinket(o.card_id),
                    cost: o.cost,
                })
                .collect(),
            dark_discovery: state
                .dark_gifts_enabled
                .then(|| [dark_gifts::uses_left(state), dark_gifts::USES_PER_GAME]),
        }
    }
}

impl ActionView {
    /// The structured `action`, taken in `state`. Classified like [`short_action`].
    fn new(state: &TavernState, action: &TavernAction) -> Self {
        match *action {
            TavernAction::Buy { shop_index } => Self::Buy { shop: shop_index },
            TavernAction::Play {
                hand_index,
                board_pos,
            } => match state.hand.get(hand_index) {
                Some(card) if card.is_spell => Self::Cast {
                    hand: hand_index,
                    target: spells::spell_requires_board_target(card.card_id).then_some(board_pos),
                },
                Some(card)
                    if card.magnetic
                        && state
                            .board
                            .get(board_pos)
                            .is_some_and(|target| target.tribe.matches(card.tribe)) =>
                {
                    Self::Magnetize {
                        hand: hand_index,
                        board: board_pos,
                    }
                }
                _ => Self::Play {
                    hand: hand_index,
                    board: board_pos,
                },
            },
            TavernAction::PlayOnShop {
                hand_index,
                shop_index,
            } => Self::CastOnShop {
                hand: hand_index,
                shop: shop_index,
            },
            TavernAction::Sell { board_pos } => Self::Sell { board: board_pos },
            TavernAction::Reposition { from_pos, to_pos } => Self::Move {
                from: from_pos,
                to: to_pos,
            },
            TavernAction::Activate {
                board_pos,
                target_pos,
            } => {
                let kind = state
                    .board
                    .get(board_pos)
                    .map(|u| cards::activate_target_kind(u.card_id));
                let zone = match kind {
                    Some(ActivateTargetKind::HandCard) => "hand",
                    Some(ActivateTargetKind::ShopCard) => "shop",
                    _ => "board",
                };
                Self::Activate {
                    board: board_pos,
                    target: target_pos,
                    zone: target_pos.map(|_| zone),
                }
            }
            TavernAction::Refresh => Self::Refresh,
            TavernAction::UpgradeTavern => Self::Upgrade,
            TavernAction::ToggleFreeze => Self::Freeze {
                frozen: !state.is_frozen,
            },
            TavernAction::ChooseDiscover { option_index } => Self::Discover {
                option: option_index,
            },
            TavernAction::EndTurn => Self::EndTurn,
            TavernAction::Pass { hand_index } => Self::Pass { hand: hand_index },
            TavernAction::HeroPower { target } => Self::HeroPower {
                target,
                second: false,
            },
            TavernAction::HeroPower2 { target } => Self::HeroPower {
                target,
                second: true,
            },
            TavernAction::ChooseHero { option_index } => Self::ChooseHero {
                option: option_index,
            },
            TavernAction::ChooseTrinket { option_index } => Self::ChooseTrinket {
                option: option_index,
            },
            TavernAction::DarkDiscovery => Self::DarkDiscovery,
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
            } else if u.locked {
                notes.push("locked".to_string());
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
            } else if u.locked {
                notes.push("locked".to_string());
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
        // The throwaway pool and RNG for `PlanSteps`, built only when a plan needs them.
        let mut scratch = None;
        let mut candidates: Vec<CandidateView> = note
            .candidates
            .iter()
            .map(|candidate| {
                let decoded = candidate
                    .action
                    .as_ref()
                    .and_then(|a| replay::action_from_pb(a).ok());
                let plan: Vec<TavernAction> = candidate
                    .plan
                    .iter()
                    .filter_map(|a| replay::action_from_pb(a).ok())
                    .collect();
                let plan = match decoded {
                    Some(first) if !plan.is_empty() => {
                        let (pool, rng) = scratch.get_or_insert_with(|| {
                            (CardPool::new(cards::full_catalog()), Rng::new(0))
                        });
                        plan_labels(state, first, &plan, pool, rng)
                    }
                    _ => Vec::new(),
                };
                CandidateView {
                    action: decoded.map_or_else(|| "?".to_string(), |a| short_action(state, &a)),
                    // Four decimals are plenty for a bar and a percentage.
                    prob: candidate.prob.map(|p| (p * 1e4).round() / 1e4),
                    score: candidate.score.map(|s| (s * 1e3).round() / 1e3),
                    label: (!candidate.label.is_empty()).then(|| candidate.label.clone()),
                    plan,
                    chosen: decoded.as_ref() == Some(action),
                }
            })
            .collect();
        if candidates.iter().any(|c| c.score.is_some()) {
            // Unscored candidates (if any) go last.
            let key = |c: &CandidateView| c.score.unwrap_or(f32::NEG_INFINITY);
            candidates.sort_by(|x, y| key(y).total_cmp(&key(x)));
        } else {
            let key = |c: &CandidateView| c.prob.unwrap_or(f32::NEG_INFINITY);
            candidates.sort_by(|x, y| key(y).total_cmp(&key(x)));
        }
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

/// A description of each step of `plan`, the line `first` starts, as the actor planned it from
/// `state`. Steps are named on the state they are taken in, found by stepping a clone. A step
/// that changes what the actor saw (a Refresh, Dark Discovery or a Hero Power may draw new cards;
/// a Discover waiting for a choice; any step whose outcome depends on the random draw, such as a
/// spell that adds a random minion to the hand) ends that: the actor planned the rest on its own
/// sampled outcome, so the later cards would be named wrongly. The rest of the steps, and any
/// step that is illegal here, are named by index only ([`index_action`]).
fn plan_labels(
    state: &TavernState,
    first: TavernAction,
    plan: &[TavernAction],
    pool: &mut CardPool,
    rng: &mut Rng,
) -> Vec<String> {
    let mut sim = Some(state.clone());
    let mut label = |action: TavernAction| -> String {
        let Some(current) = sim.as_mut() else {
            return index_action(&action);
        };
        if !current.is_legal(&action) {
            sim = None;
            return index_action(&action);
        }
        let text = short_action(current, &action);
        let reshuffles = matches!(
            action,
            TavernAction::Refresh
                | TavernAction::DarkDiscovery
                | TavernAction::HeroPower { .. }
                | TavernAction::HeroPower2 { .. }
        );
        if reshuffles {
            sim = None;
            return text;
        }
        // Step clones with other draws too: if any differs, the outcome was random. (A random pick
        // from a few options can coincide, hence several seeds.)
        let alts: Vec<Option<Vec<(u32, i32, i32)>>> = [
            0x5DEE_CE66D_u64,
            0x2545_F491_4F6C_DD1D,
            0x9E37_79B9_7F4A_7C15,
            0xD1B5_4A32_D192_ED03,
        ]
        .iter()
        .map(|&seed| {
            let mut alt = current.clone();
            let mut alt_pool = pool.clone();
            let mut alt_rng = Rng::new(seed);
            alt.step(action, &mut alt_pool, &mut alt_rng)
                .ok()
                .map(|_| visible_cards(&alt))
        })
        .collect();
        if current.step(action, pool, rng).is_err()
            || current.discover_pending.is_some()
            || alts
                .iter()
                .any(|v| v.as_ref() != Some(&visible_cards(current)))
        {
            sim = None;
        }
        text
    };
    label(first);
    plan.iter().map(|&a| label(a)).collect()
}

/// The cards a step can change, to tell a random outcome from a fixed one ([`plan_labels`]).
fn visible_cards(state: &TavernState) -> Vec<(u32, i32, i32)> {
    let cards = |units: &[Unit]| {
        units
            .iter()
            .map(|u| (u.card_id, u.attack, u.health))
            .collect::<Vec<_>>()
    };
    let mut out = cards(&state.board);
    out.push((u32::MAX, 0, 0));
    out.extend(cards(&state.hand));
    out.push((u32::MAX, 0, 0));
    out.extend(cards(&state.shop));
    out
}

/// A description of `action` that names no card, only positions (`Buy shop[2]`).
fn index_action(action: &TavernAction) -> String {
    let on_board = |t: &Option<usize>| t.map_or(String::new(), |t| format!(" on board[{t}]"));
    match *action {
        TavernAction::Buy { shop_index } => format!("Buy shop[{shop_index}]"),
        TavernAction::Play {
            hand_index,
            board_pos,
        } => format!("Play hand[{hand_index}] at {board_pos}"),
        TavernAction::PlayOnShop {
            hand_index,
            shop_index,
        } => format!("Cast hand[{hand_index}] on shop[{shop_index}]"),
        TavernAction::Sell { board_pos } => format!("Sell board[{board_pos}]"),
        TavernAction::Reposition { from_pos, to_pos } => {
            format!("Move board[{from_pos}] to {to_pos}")
        }
        TavernAction::Activate {
            board_pos,
            target_pos,
        } => format!("Activate board[{board_pos}]{}", on_board(&target_pos)),
        TavernAction::Refresh => "Refresh".to_string(),
        TavernAction::UpgradeTavern => "Upgrade tavern".to_string(),
        TavernAction::ToggleFreeze => "Toggle freeze".to_string(),
        TavernAction::ChooseDiscover { option_index } => format!("Choose option {option_index}"),
        TavernAction::EndTurn => "End turn".to_string(),
        TavernAction::Pass { hand_index } => format!("Pass hand[{hand_index}]"),
        TavernAction::HeroPower { target } | TavernAction::HeroPower2 { target } => {
            format!("Hero power{}", on_board(&target))
        }
        TavernAction::ChooseHero { option_index } => format!("Choose hero {option_index}"),
        TavernAction::ChooseTrinket { option_index } => format!("Choose trinket {option_index}"),
        TavernAction::DarkDiscovery => "Dark Discovery".to_string(),
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
                                prob: Some(prob),
                                ..Default::default()
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
                | FrameKind::RecordDeaths { .. }
                | FrameKind::DuosCombat { .. }
                | FrameKind::GhostCombat { .. } => "lobby",
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

    /// A search's note: scored candidates with the lines they planned.
    #[test]
    fn scored_candidates_carry_their_plans() {
        let player = |name: &str| PlayerSetup {
            name: name.to_string(),
            ..Default::default()
        };
        let mut game = Match::new(&MatchSetup {
            pool: PoolMode::Shared,
            shared_seed: 11,
            players: vec![player("P1"), player("P2")],
            ..Default::default()
        })
        .unwrap();
        game.start_turn(0);
        let candidate = |action, score, plan: &[TavernAction], label: &str| pb::Candidate {
            action: Some(replay::action_to_pb(&action)),
            score,
            plan: plan.iter().map(replay::action_to_pb).collect(),
            label: label.to_string(),
            ..Default::default()
        };
        let buy = TavernAction::Buy { shop_index: 0 };
        let shop_name = game.player(0).shop[0].name.clone();
        let note = pb::Note {
            value: Some(1.5),
            candidates: vec![
                candidate(TavernAction::EndTurn, Some(2.0), &[], ""),
                candidate(
                    buy,
                    Some(245.5678),
                    &[
                        TavernAction::ToggleFreeze,
                        TavernAction::Refresh,
                        TavernAction::Buy { shop_index: 2 },
                    ],
                    "plan",
                ),
            ],
            ..Default::default()
        };
        game.step_with_note(0, buy, Some(note)).unwrap();
        let timeline = Timeline::new(&game.replay()).unwrap();
        let data = embedded_data(&render(&timeline, "test.replay.pb"));
        let note = data["frames"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|f| f["note"].is_object().then(|| f["note"].clone()))
            .expect("the action has a note");
        let candidates = note["candidates"].as_array().unwrap();
        // Best score first; the line's steps are named on the state they are taken in until the
        // Refresh, then by position.
        assert_eq!(candidates[0]["action"], format!("Buy {shop_name}"));
        assert_eq!(candidates[0]["score"], 245.568);
        assert_eq!(candidates[0]["label"], "plan");
        assert_eq!(candidates[0]["chosen"], true);
        assert!(candidates[0].get("prob").is_none());
        assert_eq!(
            candidates[0]["plan"],
            serde_json::json!(["Freeze", "Refresh", "Buy shop[2]"])
        );
        assert_eq!(candidates[1]["action"], "End turn");
        assert!(candidates[1].get("plan").is_none());
        assert!(candidates[1].get("label").is_none());
    }

    #[test]
    fn the_template_has_one_data_placeholder() {
        assert_eq!(TEMPLATE.matches(DATA).count(), 1);
        assert!(TEMPLATE.contains(&format!(
            r#"<script type="application/json" id="replay-data">{DATA}</script>"#
        )));
    }

    #[test]
    fn the_player_template_has_one_data_placeholder() {
        assert_eq!(PLAYER_TEMPLATE.matches(DATA).count(), 1);
        assert!(PLAYER_TEMPLATE.contains(&format!(
            r#"<script type="application/json" id="replay-data">{DATA}</script>"#
        )));
    }

    #[test]
    fn act_frames_carry_their_action() {
        let timeline = Timeline::new(&recorded_match(7)).unwrap();
        let data = embedded_data(&render(&timeline, "test.replay.pb"));
        let frames = data["frames"].as_array().unwrap();
        let mut acts = 0;
        for (i, (frame, json)) in timeline.frames.iter().zip(frames).enumerate() {
            let FrameKind::Act { action, .. } = &frame.kind else {
                assert!(json.get("action").is_none(), "frame {i}");
                continue;
            };
            acts += 1;
            let a = &json["action"];
            let state = &timeline.frames[i - 1].players[frame_player(&frame.kind)];
            let ty = a["type"].as_str().unwrap();
            let idx = |key: &str| a[key].as_u64().map(|n| n as usize);
            let opt = |key: &str| {
                assert!(a.get(key).is_some(), "frame {i}: `{key}` is always there");
                a[key].as_u64().map(|n| n as usize)
            };
            match *action {
                TavernAction::Buy { shop_index } => {
                    assert_eq!((ty, idx("shop")), ("buy", Some(shop_index)));
                }
                TavernAction::Play {
                    hand_index,
                    board_pos,
                } => {
                    let card = &state.hand[hand_index];
                    if card.is_spell {
                        let target =
                            spells::spell_requires_board_target(card.card_id).then_some(board_pos);
                        assert_eq!((ty, opt("target")), ("cast", target), "frame {i}");
                        assert_eq!(idx("hand"), Some(hand_index));
                    } else {
                        assert!(ty == "play" || ty == "magnetize", "frame {i}: {ty}");
                        if ty == "magnetize" {
                            assert!(card.magnetic);
                        }
                        assert_eq!(idx("hand"), Some(hand_index));
                        assert_eq!(idx("board"), Some(board_pos));
                    }
                }
                TavernAction::PlayOnShop {
                    hand_index,
                    shop_index,
                } => {
                    assert_eq!(ty, "cast_on_shop");
                    assert_eq!(
                        (idx("hand"), idx("shop")),
                        (Some(hand_index), Some(shop_index))
                    );
                }
                TavernAction::Sell { board_pos } => {
                    assert_eq!((ty, idx("board")), ("sell", Some(board_pos)));
                }
                TavernAction::Reposition { from_pos, to_pos } => {
                    assert_eq!(ty, "move");
                    assert_eq!((idx("from"), idx("to")), (Some(from_pos), Some(to_pos)));
                }
                TavernAction::Activate {
                    board_pos,
                    target_pos,
                } => {
                    assert_eq!((ty, idx("board")), ("activate", Some(board_pos)));
                    assert_eq!(opt("target"), target_pos);
                    assert_eq!(a["zone"].is_string(), target_pos.is_some());
                }
                TavernAction::Refresh => assert_eq!(ty, "refresh"),
                TavernAction::UpgradeTavern => assert_eq!(ty, "upgrade"),
                TavernAction::ToggleFreeze => {
                    assert_eq!(ty, "freeze");
                    assert_eq!(a["frozen"], !state.is_frozen);
                }
                TavernAction::ChooseDiscover { option_index } => {
                    assert_eq!((ty, idx("option")), ("discover", Some(option_index)));
                }
                TavernAction::EndTurn => assert_eq!(ty, "end_turn"),
                TavernAction::Pass { hand_index } => {
                    assert_eq!((ty, idx("hand")), ("pass", Some(hand_index)));
                }
                TavernAction::HeroPower { target } | TavernAction::HeroPower2 { target } => {
                    assert_eq!(ty, "hero_power");
                    assert_eq!(opt("target"), target);
                    assert_eq!(
                        a["second"],
                        matches!(action, TavernAction::HeroPower2 { .. })
                    );
                }
                TavernAction::ChooseHero { option_index } => {
                    assert_eq!((ty, idx("option")), ("choose_hero", Some(option_index)));
                }
                TavernAction::ChooseTrinket { option_index } => {
                    assert_eq!((ty, idx("option")), ("choose_trinket", Some(option_index)));
                }
                TavernAction::DarkDiscovery => assert_eq!(ty, "dark_discovery"),
            }
        }
        assert!(acts > 0);
    }

    /// The player of an `Act` frame.
    fn frame_player(kind: &FrameKind) -> usize {
        match kind {
            FrameKind::Act { player, .. } => *player,
            _ => unreachable!("not an action"),
        }
    }

    #[test]
    fn the_page_has_the_rules_text_it_shows() {
        let timeline = Timeline::new(&recorded_match(7)).unwrap();
        let data = embedded_data(&render(&timeline, "test.replay.pb"));
        let texts = data["card_text"].as_object().unwrap();
        assert!(!texts.is_empty());
        for unit in data["units"].as_array().unwrap() {
            let card = unit["card"].as_u64().unwrap() as CardId;
            let text = texts.get(&card.to_string()).and_then(Value::as_str);
            assert_eq!(text, card_text(card), "{}", unit["name"]);
        }
        // Every trinket a state names has its text; Dark Discovery counts its uses.
        for frame in data["frames"].as_array().unwrap() {
            for state in frame["states"].as_array().unwrap() {
                let owned = state["trinkets"].as_array().into_iter().flatten();
                let offered = state["trinket_options"].as_array().into_iter().flatten();
                for name in owned.chain(offered.map(|o| &o["name"])) {
                    let key = format!("trinket:{}", name.as_str().unwrap());
                    assert!(data["texts"][key.as_str()].is_string(), "{key}");
                }
                if let Some(button) = state["dark_discovery"].as_array() {
                    let left = button[0].as_u64().unwrap();
                    assert!(left <= 3 && button[1] == 3, "{button:?}");
                }
            }
        }
    }

    #[test]
    fn trinkets_and_heroes_note_their_text() {
        let mut units = Units::default();
        let trinket = &trinkets::TRINKETS[0];
        let hero = &heroes::HEROES[0];
        assert_eq!(units.trinket(trinket.id), trinket.name);
        assert_eq!(units.hero(hero.id), hero.name);
        assert_eq!(units.trinket(CardId::MAX), "?");
        let text = |key: String| units.texts.get(&key).cloned();
        assert_eq!(
            text(format!("trinket:{}", trinket.name)),
            Some(trinket.text.to_string())
        );
        assert_eq!(
            text(format!("power:{}", hero.power.name)),
            Some(hero.power.text.to_string())
        );
        let hero_text = text(format!("hero:{}", hero.name)).unwrap();
        assert!(hero_text.starts_with(&format!("Hero Power: {} (", hero.power.name)));
        assert!(hero_text.ends_with(hero.power.text));
    }

    #[test]
    fn the_player_page_is_the_page_with_a_focus() {
        let timeline = Timeline::new(&recorded_match(7)).unwrap();
        let normal = embedded_data(&render(&timeline, "test.replay.pb"));
        assert!(normal.get("focus").is_none());
        let page = render_player(&timeline, "test.replay.pb", 1);
        assert!(!page.contains(DATA));
        let mut player = embedded_data(&page);
        assert_eq!(player["focus"], 1);
        player.as_object_mut().unwrap().remove("focus");
        assert_eq!(player, normal);
        // Every player view says what a refresh costs.
        let first = &normal["frames"][0]["states"][0];
        assert!(first["refresh_cost"].is_u64());
    }

    #[test]
    fn the_player_page_goes_next_to_the_replay() {
        let out = |input: &str, p| default_player_output(Path::new(input), p);
        assert_eq!(
            out("replays/s7.replay.pb", 0),
            Path::new("replays/s7.p0.html")
        );
        assert_eq!(out("game.pb", 0), Path::new("game.p0.html"));
        assert_eq!(
            out("runs/v1.2/game", 3),
            Path::new("runs/v1.2/game.p3.html")
        );
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
