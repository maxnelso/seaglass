//! Invariant fuzzer over full random Solo games (`docs/tavern.md`, `docs/combat.md`).
//!
//! 2 to 8 players share one `CardPool` (the `full` catalog, sometimes a smaller one) and play
//! random Tavern turns through the public API: `start_turn`, then actions picked from
//! `valid_actions()` (biased toward buying, playing, selling and spells) until `EndTurn` or an
//! action cap. Then they fight in pairs (`TavernState::resolve_combat_pair`; the odd player out
//! fights the last eliminated player's ghost with `resolve_combat_against`), and players at 0
//! Health or less are eliminated, until one is left or the turn cap. Some games give the players
//! extra maximum Gold or random cards first (Tier 7 minions, token spells), so that every card
//! gets played.
//!
//! After every action, start of turn and combat it checks:
//! - [`check_legality`]: `valid_actions()` lists each legal action once, `is_legal` agrees with it
//!   on every candidate action (each index up to one past its zone, and a huge one; a Duos `Pass`
//!   too, which Solo never allows), a pending Discover allows only `ChooseDiscover`, and `step()`
//!   accepts every listed action (all of them, on copies of the game, at
//!   `SEAGLASS_FUZZ_LOOKAHEAD` of the decision points);
//! - [`check_tavern`]: at most 7 minions on the board and 10 cards in hand, a Tier in 1..=6,
//!   minions in the Tavern with at least 1 Health and no negative Attack, no spell on the board,
//!   no empty or orphaned Discover, `max_gold`, `upgrade_cost` and Armor as specified, no three
//!   non-Golden copies left untripled, each unit matching its card (Tier, type, name, base
//!   stats), no leftover marker id or transient state;
//! - [`check_synced`]: re-syncing the auras changes nothing; [`check_round_trip`]: the state
//!   survives a serde round trip; [`check_pool_leaks`]: the pool holds at most the base copies
//!   and no copy leaves it for nowhere;
//! - per action: a rolled shop holds `shop_capacity` minions and one spell, prices are paid in
//!   Gold, a Health cost never kills, Upgrade goes up one Tier, a bought or Discovered card
//!   reaches the hand, a Golden played from hand offers a Triple Reward of the next Tier, a
//!   Tavern spell counts as cast, the hero is never healed;
//! - per combat: [`check_battle`] (at most 7 survivors, all alive, outcome and `hero_damage` as
//!   specified, events framed by `BattleStart` / `BattleEnd`), [`check_battle_units`] (every unit
//!   accounted for by the events), the hero loses exactly the damage, and the Tavern keeps its
//!   minions, shop, Gold and Tier ([`check_combat_keeps_tavern`]).
//!
//! A panic is caught per game, so one run collects several failures, and every game is replayed
//! from its seed to check that it is deterministic. Failures are grouped by signature; the
//! earliest one of each group is kept as its repro (with `SEAGLASS_FUZZ_OUT`, written as a
//! snapshot of the state just before the failing call, which `SEAGLASS_FUZZ_REPRO` replays).
//! Games go on past the failures of known bugs ([`KNOWN_BUGS`], reported in
//! `tests/scenarios/tavern/fuzz_findings.yaml`), and `cargo test` fails only on new ones.
//! [`random_battles_keep_the_invariants`] checks the same combat invariants over random boards.
//!
//! - `SEAGLASS_FUZZ_GAMES`: the number of games (default 12).
//! - `SEAGLASS_FUZZ_BATTLES`: the number of random battles (default 300).
//! - `SEAGLASS_FUZZ_SEED`: the first game's (or battle's) seed (default 1); game `i` uses
//!   `seed + i`.
//! - `SEAGLASS_FUZZ_THREADS`: worker threads (default: up to 4).
//! - `SEAGLASS_FUZZ_TURNS`: the turn cap, varied per game in `turns/2..=turns*3/2` (default 25).
//! - `SEAGLASS_FUZZ_LOOKAHEAD`: per mille of decision points at which every listed action is
//!   also stepped on a copy of the game (default 100; use the run's value to trace its games).
//! - `SEAGLASS_FUZZ_GAME=<seed>`: play that one game, printing its trace (and checking the pool
//!   after every action, to find the one that leaks).
//! - `SEAGLASS_FUZZ_OUT=<dir>`: write each failure group's repro snapshot there.
//! - `SEAGLASS_FUZZ_REPRO=<file>`: replay a repro snapshot's failing call, printing the result
//!   (a combat's events too).
//! - `SEAGLASS_FUZZ_STRICT=1`: fail on the known bugs too.
//! - `SEAGLASS_FUZZ_STOP_AT_KNOWN=1`: end a game at a known bug instead of going on.
//!
//! A big run, in release with the debug assertions and overflow checks on:
//!
//! ```sh
//! CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=true \
//! SEAGLASS_FUZZ_GAMES=50000 SEAGLASS_FUZZ_BATTLES=3000000 SEAGLASS_FUZZ_THREADS=18 \
//! SEAGLASS_FUZZ_LOOKAHEAD=300 SEAGLASS_FUZZ_OUT=target/fuzz \
//!   cargo test --release -p seaglass --test tavern_fuzz -- --nocapture
//! ```

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

use seaglass::cards::{self, CardFlags};
use seaglass::{
    base_copies_for_tier, base_upgrade_cost, catalog_for, shop_capacity, BattleOutcome,
    BattleResult, CardPool, CardTemplate, DeityKind, Event, PlayerAuras, Rng, Side, TavernAction,
    TavernState, Unit, UnitId,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------------------------

/// Worker threads by default: a few, so that `cargo test` stays fast.
fn default_threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().min(4))
}

fn env_or<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Run-wide settings, from the environment.
#[derive(Clone, Debug)]
struct Settings {
    games: u64,
    first_seed: u64,
    threads: usize,
    turns: u32,
    lookahead: u64,
    out: Option<String>,
    strict: bool,
    /// Go on past the failures of known bugs (`SEAGLASS_FUZZ_STOP_AT_KNOWN=1` stops there).
    continue_known: bool,
}

impl Settings {
    fn from_env() -> Self {
        Self {
            games: env_or("SEAGLASS_FUZZ_GAMES", 12),
            first_seed: env_or("SEAGLASS_FUZZ_SEED", 1),
            threads: env_or("SEAGLASS_FUZZ_THREADS", default_threads()).max(1),
            turns: env_or("SEAGLASS_FUZZ_TURNS", 25),
            lookahead: env_or("SEAGLASS_FUZZ_LOOKAHEAD", 100),
            out: std::env::var("SEAGLASS_FUZZ_OUT").ok(),
            strict: std::env::var("SEAGLASS_FUZZ_STRICT").is_ok_and(|v| v == "1"),
            continue_known: !std::env::var("SEAGLASS_FUZZ_STOP_AT_KNOWN").is_ok_and(|v| v == "1"),
        }
    }
}

/// The catalogs a game can use besides `full`.
const SMALL_CATALOGS: [&str; 6] = [
    "tier1", "tier1_2", "tier1_3", "tier1_4", "tier1_5", "tier1_6",
];

/// One game's setup, drawn from its seed.
#[derive(Clone, Debug)]
struct Config {
    players: usize,
    catalog: &'static str,
    shop_spells: bool,
    max_turns: u32,
    /// Actions a player takes at most in one Tavern turn before ending it.
    action_cap: usize,
    /// The policy's weight for each [`Kind`] of action.
    weights: [u32; KINDS],
    armor: Vec<i32>,
    deities: Vec<DeityKind>,
    /// Extra maximum Gold every player starts with (as from `Strike Oil`s), so that some games
    /// reach high Tiers and full boards early.
    rich: u32,
    /// Cards given to players before the game (`add_to_hand`, as if Discovered), so that rare
    /// cards (Tier 7 minions, token spells) get played too: `(player, card)`.
    gifts: Vec<(usize, Unit)>,
}

impl Config {
    fn random(seed: u64, turns: u32) -> Self {
        let mut rng = Rng::new(seed ^ 0xC0F1_6000_5EED_0001);
        let players = 2 + rng.below(7);
        let catalog = if rng.below(5) == 0 {
            SMALL_CATALOGS[rng.below(SMALL_CATALOGS.len())]
        } else {
            "full"
        };
        let shop_spells = rng.below(10) != 0;
        let action_cap = 30 + rng.below(50);
        let mut weights = BASE_WEIGHTS;
        // A kind with a base weight of 0 (Pass) is never taken and draws nothing, so that adding
        // it left every seed's game as it was.
        for w in weights.iter_mut().filter(|w| **w > 0) {
            // Between half and twice the base weight, so games lean different ways.
            *w = (*w * (2 + rng.below(7) as u32) / 4).max(1);
        }
        let armor = (0..players).map(|_| rng.below(11) as i32).collect();
        let deities = (0..players)
            .map(|_| [DeityKind::CThun, DeityKind::YShaarj, DeityKind::None][rng.below(3)])
            .collect();
        let rich = if rng.below(4) == 0 {
            3 + rng.below(13) as u32
        } else {
            0
        };
        let mut gifts = Vec::new();
        if rng.below(2) == 0 {
            let minions = cards::all_templates();
            let spells = cards::spells::all_spells();
            for player in 0..players {
                for _ in 0..1 + rng.below(3) {
                    let card = if rng.below(5) < 3 {
                        let mut unit = minions[rng.below(minions.len())].instantiate();
                        if rng.below(5) == 0 {
                            unit.make_golden();
                        }
                        unit
                    } else {
                        spells[rng.below(spells.len())].clone()
                    };
                    gifts.push((player, card));
                }
            }
        }
        Self {
            players,
            catalog,
            shop_spells,
            max_turns: turns / 2 + rng.below(turns as usize + 1) as u32,
            action_cap,
            weights,
            armor,
            deities,
            rich,
            gifts,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The random policy
// ---------------------------------------------------------------------------------------------

/// What an action does, for the policy's weights.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    BuyMinion,
    BuySpell,
    PlayMinion,
    PlaySpell,
    PlayOnShop,
    Sell,
    Reposition,
    Activate,
    Refresh,
    Upgrade,
    Freeze,
    EndTurn,
    Discover,
    /// A Duos Pass, which a Solo game never allows.
    Pass,
}

const KINDS: usize = 14;

/// Base weights, indexed by `Kind as usize`: mostly buying and playing, so games develop.
const BASE_WEIGHTS: [u32; KINDS] = [30, 15, 40, 30, 12, 8, 2, 15, 6, 8, 2, 3, 1, 0];

fn kind_of(s: &TavernState, action: &TavernAction) -> Kind {
    match *action {
        TavernAction::Buy { shop_index } => {
            if s.shop.get(shop_index).is_some_and(|u| u.is_spell) {
                Kind::BuySpell
            } else {
                Kind::BuyMinion
            }
        }
        TavernAction::Play { hand_index, .. } => {
            if s.hand.get(hand_index).is_some_and(|u| u.is_spell) {
                Kind::PlaySpell
            } else {
                Kind::PlayMinion
            }
        }
        TavernAction::PlayOnShop { .. } => Kind::PlayOnShop,
        TavernAction::Sell { .. } => Kind::Sell,
        TavernAction::Reposition { .. } => Kind::Reposition,
        TavernAction::Activate { .. } => Kind::Activate,
        TavernAction::Refresh => Kind::Refresh,
        TavernAction::UpgradeTavern => Kind::Upgrade,
        TavernAction::ToggleFreeze => Kind::Freeze,
        TavernAction::EndTurn => Kind::EndTurn,
        TavernAction::ChooseDiscover { .. } => Kind::Discover,
        TavernAction::Pass { .. } => Kind::Pass,
    }
}

/// Pick one of the `valid` actions: a kind by weight, then an action of that kind uniformly.
fn choose(
    s: &TavernState,
    valid: &[TavernAction],
    weights: &[u32; KINDS],
    steps: usize,
    rng: &mut Rng,
) -> TavernAction {
    let mut by_kind: Vec<(Kind, Vec<TavernAction>)> = Vec::new();
    for a in valid {
        let kind = kind_of(s, a);
        match by_kind.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, list)) => list.push(*a),
            None => by_kind.push((kind, vec![*a])),
        }
    }
    let full_board = s.board.len() >= 7 && s.hand.iter().any(|u| !u.is_spell);
    let weight = |kind: Kind| -> u32 {
        let w = weights[kind as usize];
        match kind {
            Kind::Sell if full_board => w * 4,
            Kind::EndTurn => w + steps as u32 / 4,
            _ => w,
        }
    };
    let total: u32 = by_kind.iter().map(|(k, _)| weight(*k)).sum();
    let mut pick = rng.below(total as usize) as u32;
    for (kind, list) in &by_kind {
        let w = weight(*kind);
        if pick < w {
            return list[rng.below(list.len())];
        }
        pick -= w;
    }
    unreachable!("the pick lands in a kind")
}

// ---------------------------------------------------------------------------------------------
// Failures
// ---------------------------------------------------------------------------------------------

/// How a failure breaks the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
enum FailureKind {
    Crash,
    IllegalActionAccepted,
    LegalActionRejected,
    BrokenInvariant,
    Nondeterminism,
}

/// A broken invariant: its kind, a message without the varying details (the group it belongs
/// to) and the details.
#[derive(Clone, Debug)]
struct Violation {
    kind: FailureKind,
    what: String,
    detail: String,
}

fn violation(kind: FailureKind, what: impl Into<String>, detail: impl Into<String>) -> Violation {
    Violation {
        kind,
        what: what.into(),
        detail: detail.into(),
    }
}

fn broken(what: impl Into<String>, detail: impl Into<String>) -> Violation {
    violation(FailureKind::BrokenInvariant, what, detail)
}

/// The phase of the game an engine call belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Phase {
    StartTurn,
    /// A decision point: the checks of the state and of its actions, then the chosen action.
    Action,
    /// A listed action stepped on a copy of the game (see `SEAGLASS_FUZZ_LOOKAHEAD`).
    Lookahead,
    /// An action that `is_legal` accepts but `valid_actions` doesn't list, stepped on a copy.
    Unlisted,
    Combat,
    Ghost,
}

/// What the current engine call is, for failures (panics included).
#[derive(Clone, Debug)]
struct Call {
    index: u64,
    turn: u32,
    phase: Phase,
    player: usize,
    opponent: Option<usize>,
    action: Option<TavernAction>,
}

/// One failure of one game.
#[derive(Clone, Debug)]
struct Failure {
    seed: u64,
    kind: FailureKind,
    /// The failure's group: its message with the numbers left out.
    signature: String,
    detail: String,
    call: Call,
    /// The cards involved: the acting card and the target, if any.
    cards: String,
}

/// `text` with every run of digits replaced by `#`.
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_digits = false;
    for c in text.chars() {
        if c.is_ascii_digit() {
            if !in_digits {
                out.push('#');
            }
            in_digits = true;
        } else {
            out.push(c);
            in_digits = false;
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Panic capture
// ---------------------------------------------------------------------------------------------

thread_local! {
    /// Set while a game runs on this thread: its panics are recorded, not printed.
    static CAPTURING: RefCell<bool> = const { RefCell::new(false) };
    static LAST_PANIC: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn install_panic_hook() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let default = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if CAPTURING.with(|c| *c.borrow()) {
                let location = info
                    .location()
                    .map(|l| format!("{}:{}", l.file(), l.line()))
                    .unwrap_or_default();
                let payload = info.payload();
                let message = payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "<non-string panic>".to_string());
                let mut text = format!("{location}: {message}");
                if std::env::var("SEAGLASS_FUZZ_BACKTRACE").is_ok() {
                    let _ = write!(text, "\n{}", std::backtrace::Backtrace::force_capture());
                }
                LAST_PANIC.with(|p| *p.borrow_mut() = Some(text));
            } else {
                default(info);
            }
        }));
    });
}

// ---------------------------------------------------------------------------------------------
// Invariants
// ---------------------------------------------------------------------------------------------

fn unit_desc(u: &Unit) -> String {
    let golden = if u.is_golden { "golden " } else { "" };
    if u.is_spell {
        format!("spell {}", u.name)
    } else {
        format!("{golden}{} {}/{}", u.name, u.attack, u.health)
    }
}

fn units_desc(units: &[Unit]) -> String {
    let names: Vec<String> = units.iter().map(unit_desc).collect();
    format!("[{}]", names.join(", "))
}

/// The checks of one Tavern state, between engine calls (`docs/tavern.md` §2, §3, §5, §6).
fn check_tavern(s: &TavernState) -> Result<(), Violation> {
    if !(1..=6).contains(&s.tavern_tier) {
        return Err(broken(
            "Tavern Tier out of 1..=6",
            format!("tier {}", s.tavern_tier),
        ));
    }
    if s.board.len() > 7 {
        return Err(broken(
            format!("board has {} minions", s.board.len()),
            units_desc(&s.board),
        ));
    }
    if s.hand.len() > 10 {
        return Err(broken(
            format!("hand has {} cards", s.hand.len()),
            units_desc(&s.hand),
        ));
    }
    // A rolled shop holds `shop_capacity` minions (`check_refilled_shop`); the tokens effects add
    // to the Tavern (`Demon Fodder`, `Fishbait`) come on top, and an effect can turn one into a
    // pool minion (`Robust Evolution`), so the bound here is loose.
    let cap = shop_capacity(s.tavern_tier);
    if s.shop.len() > cap + 1 + 8 {
        return Err(broken(
            format!(
                "shop has {} cards at Tier {} (capacity {cap})",
                s.shop.len(),
                s.tavern_tier
            ),
            units_desc(&s.shop),
        ));
    }
    if let Some(u) = s.board.iter().find(|u| u.is_spell) {
        return Err(broken(
            format!("a spell is on the board: {}", u.name),
            units_desc(&s.board),
        ));
    }
    for (zone, units) in [("board", &s.board), ("hand", &s.hand), ("shop", &s.shop)] {
        for u in units.iter().filter(|u| !u.is_spell) {
            if u.health < 1 {
                return Err(broken(
                    format!("a minion in the {zone} has {} Health: {}", u.health, u.name),
                    units_desc(units),
                ));
            }
            if u.attack < 0 {
                return Err(broken(
                    format!("a minion in the {zone} has {} Attack: {}", u.attack, u.name),
                    units_desc(units),
                ));
            }
        }
    }
    match &s.discover_pending {
        Some(opts) if opts.is_empty() => {
            return Err(broken("an empty Discover is pending", ""));
        }
        None if !s.discover_queue.is_empty() => {
            return Err(broken(
                "a Discover is queued but none is pending",
                format!("{} queued", s.discover_queue.len()),
            ));
        }
        _ => {}
    }
    if s.turn >= 1 {
        let expected = (2 + s.turn).min(10) + s.auras.base_max_gold_bonus;
        if s.max_gold != expected {
            return Err(broken(
                format!(
                    "max_gold is {} on turn {} (expected {expected})",
                    s.max_gold, s.turn
                ),
                format!("base_max_gold_bonus {}", s.auras.base_max_gold_bonus),
            ));
        }
    }
    if s.tavern_tier == 6 && s.upgrade_cost != 0 {
        return Err(broken(
            format!("upgrade_cost is {} at Tier 6", s.upgrade_cost),
            "",
        ));
    }
    if s.tavern_tier < 6 && s.upgrade_cost > base_upgrade_cost(s.tavern_tier) {
        return Err(broken(
            format!(
                "upgrade_cost is {} at Tier {} (base {})",
                s.upgrade_cost,
                s.tavern_tier,
                base_upgrade_cost(s.tavern_tier)
            ),
            "",
        ));
    }
    if s.armor < 0 {
        return Err(broken(format!("armor is {}", s.armor), ""));
    }
    if s.gold > 1000 {
        return Err(broken(format!("gold is {}", s.gold), "an underflow?"));
    }
    check_units(s)?;
    check_transients(s)?;
    // Three non-Golden copies across board and hand form a Triple, unless its Golden has no room
    // in the hand: the copies are taken from the board first (`docs/tavern.md` §5).
    let mut copies: BTreeMap<u32, (usize, usize)> = BTreeMap::new();
    for (u, in_hand) in s
        .board
        .iter()
        .map(|u| (u, false))
        .chain(s.hand.iter().map(|u| (u, true)))
    {
        let eligible = !u.is_spell
            && !u.is_golden
            && !u.intrinsic_golden
            && u.card_id != 0
            && !cards::hooks(u.card_id).has(CardFlags::NO_TRIPLE);
        if eligible {
            let (on_board, held) = copies.entry(u.card_id).or_default();
            *if in_hand { held } else { on_board } += 1;
        }
    }
    let untripled = copies.iter().find(|(_, &(on_board, held))| {
        let from_hand = held.min(3 - on_board.min(3));
        on_board + held >= 3 && s.hand.len() - from_hand < 10
    });
    if let Some((&cid, &(on_board, held))) = untripled {
        let n = on_board + held;
        let name = s
            .board
            .iter()
            .chain(&s.hand)
            .find(|u| u.card_id == cid)
            .map(|u| u.name.clone())
            .unwrap_or_default();
        return Err(broken(
            "3 non-Golden copies of a minion did not form a Triple",
            format!(
                "{n} copies of {name}: board {} hand {}",
                units_desc(&s.board),
                units_desc(&s.hand)
            ),
        ));
    }
    Ok(())
}

/// The ids at and above this mark a minion while an effect resolves (the targets of a spell, a
/// Battlecry or a Tavern attack, the distinct ids of a Tavern Rally): none is left once it has.
const MARKER_IDS: UnitId = 1 << 30;

/// The checks of each unit in the Tavern: no leftover marker id, the identity of its card
/// (Tier, type, name and base stats, doubled for a Golden), and its maximum stats.
fn check_units(s: &TavernState) -> Result<(), Violation> {
    let prompts = s.discover_pending.iter().chain(&s.discover_queue).flatten();
    let zones = s
        .board
        .iter()
        .map(|u| ("board", u))
        .chain(s.hand.iter().map(|u| ("hand", u)))
        .chain(s.shop.iter().map(|u| ("shop", u)))
        .chain(prompts.map(|u| ("Discover", u)));
    for (zone, u) in zones {
        if u.id >= MARKER_IDS {
            return Err(broken(
                format!("a marker id is left on a card in the {zone}"),
                format!("{} has id {}", unit_desc(u), u.id),
            ));
        }
        if u.is_spell {
            continue;
        }
        if let Some(t) = cards::template(u.card_id) {
            // (A card that is always Golden has its Golden stats printed.)
            let mult = if u.is_golden && !t.intrinsic_golden {
                2
            } else {
                1
            };
            let golden_name = format!("Golden {}", t.name);
            let problem = if u.tavern_tier != t.tavern_tier {
                Some(format!("Tier {} (card: {})", u.tavern_tier, t.tavern_tier))
            } else if u.tribe != t.tribe {
                Some(format!("type {:?} (card: {:?})", u.tribe, t.tribe))
            } else if u.name != t.name && u.name != golden_name {
                Some(format!("name {:?}", u.name))
            } else if (u.base_attack, u.base_health) != (t.attack * mult, t.health * mult) {
                Some(format!(
                    "base stats {}/{} (card: {}/{}, golden {})",
                    u.base_attack, u.base_health, t.attack, t.health, u.is_golden
                ))
            } else {
                None
            };
            if let Some(problem) = problem {
                return Err(broken(
                    format!("a minion in the {zone} doesn't match its card: {}", t.name),
                    problem,
                ));
            }
        }
        if zone != "shop" && (u.max_attack < u.attack || u.max_health < u.health) {
            return Err(broken(
                format!(
                    "a minion in the {zone} is above its maximum stats: {}",
                    u.name
                ),
                format!(
                    "{}/{} with maximum {}/{}",
                    u.attack, u.health, u.max_attack, u.max_health
                ),
            ));
        }
    }
    for opts in s.discover_pending.iter().chain(&s.discover_queue) {
        for (i, u) in opts.iter().enumerate() {
            if opts[..i].iter().any(|o| o.card_id == u.card_id) {
                return Err(broken(
                    format!("a Discover offers the same card twice: {}", u.name),
                    units_desc(opts),
                ));
            }
        }
    }
    Ok(())
}

/// The state an action uses while it resolves is reset once it has: no play slot, no Triple
/// deferred, no cast being repeated, no Choose One being combined, and no Choose One target or
/// repeat without a prompt.
fn check_transients(s: &TavernState) -> Result<(), Violation> {
    let problem = if s.play_slot.is_some() {
        Some(format!("play_slot {:?}", s.play_slot))
    } else if s.triples_deferred {
        Some("triples_deferred".to_string())
    } else if s.repeating_cast {
        Some("repeating_cast".to_string())
    } else if s.combine_choose_one || s.defer_combined_choose_one {
        Some("combine_choose_one".to_string())
    } else if !s.pending_combined_choices.is_empty() {
        Some(format!(
            "{} pending_combined_choices",
            s.pending_combined_choices.len()
        ))
    } else if s.discover_pending.is_none() && s.pending_choice_target.is_some() {
        Some(format!(
            "pending_choice_target {:?}",
            s.pending_choice_target
        ))
    } else if s.discover_pending.is_none() && s.pending_choice_repeats != 0 {
        Some(format!(
            "pending_choice_repeats {}",
            s.pending_choice_repeats
        ))
    } else {
        None
    };
    match problem {
        Some(p) => Err(broken(
            format!("an action left its transient state set: {}", normalize(&p)),
            p,
        )),
        None => Ok(()),
    }
}

/// Re-syncing the "wherever this is" auras (`sync_all_auras`) changes nothing: the Tavern keeps
/// them in sync (`docs/tavern.md` §7.6).
fn check_synced(s: &TavernState) -> Result<(), Violation> {
    let mut synced = s.clone();
    synced.sync_all_auras();
    for (zone, before, after) in [
        ("board", &s.board, &synced.board),
        ("hand", &s.hand, &synced.hand),
        ("shop", &s.shop, &synced.shop),
    ] {
        if let Some((b, a)) = before.iter().zip(after).find(|(b, a)| b != a) {
            return Err(broken(
                format!(
                    "a minion in the {zone} changes when the auras are re-synced: {}",
                    b.name
                ),
                field_changes(b, a),
            ));
        }
    }
    let mut auras = synced.auras.clone();
    auras.hero_health = s.auras.hero_health;
    if auras != s.auras {
        return Err(broken(
            "the player auras change when re-synced",
            field_changes(&s.auras, &auras),
        ));
    }
    Ok(())
}

/// The fields of `before` that differ in `after`, as `field: before -> after`.
fn field_changes<T: Serialize>(before: &T, after: &T) -> String {
    let (Ok(serde_yaml::Value::Mapping(b)), Ok(serde_yaml::Value::Mapping(a))) =
        (serde_yaml::to_value(before), serde_yaml::to_value(after))
    else {
        return String::new();
    };
    let mut out = Vec::new();
    for (key, old) in &b {
        let new = a.get(key);
        if new != Some(old) {
            let show = |v: Option<&serde_yaml::Value>| {
                v.map(|v| yaml(v).trim().replace('\n', " "))
                    .unwrap_or_default()
            };
            out.push(format!(
                "{}: {} -> {}",
                show(Some(key)),
                show(Some(old)),
                show(new)
            ));
        }
    }
    out.join(", ")
}

/// Every unit of a battle is accounted for by its events (`docs/combat.md` §6): each one is on a
/// starting board (ids `0..len_a` for A, then B's) or summoned with a fresh id (`UnitSummoned`,
/// `DeityAwakened`), dies at most once (`Death`), attacks, is attacked and takes damage only
/// while alive, and the survivors are exactly the units of their side that didn't die.
fn check_battle_units(res: &BattleResult, len_a: usize, len_b: usize) -> Result<(), Violation> {
    let mut side_of: BTreeMap<UnitId, Side> = BTreeMap::new();
    for id in 0..len_a + len_b {
        side_of.insert(id as UnitId, if id < len_a { Side::A } else { Side::B });
    }
    let mut dead: BTreeSet<UnitId> = BTreeSet::new();
    let alive = |side_of: &BTreeMap<UnitId, Side>, dead: &BTreeSet<UnitId>, id: UnitId| {
        side_of.get(&id).copied().filter(|_| !dead.contains(&id))
    };
    for (i, ev) in res.events.iter().enumerate() {
        let problem = match ev {
            Event::UnitSummoned { side, unit, .. } | Event::DeityAwakened { side, unit, .. } => {
                side_of
                    .insert(*unit, *side)
                    .map(|_| format!("unit {unit} is summoned with an id already used"))
            }
            Event::Death { unit } => match alive(&side_of, &dead, *unit) {
                Some(_) => {
                    dead.insert(*unit);
                    None
                }
                None => Some(format!(
                    "unit {unit} dies, but it is unknown or already dead"
                )),
            },
            Event::AttackDeclared {
                side,
                attacker,
                target,
            } => {
                if alive(&side_of, &dead, *attacker) != Some(*side) {
                    Some(format!(
                        "unit {attacker} attacks for side {side:?}, but it isn't alive there"
                    ))
                } else if alive(&side_of, &dead, *target) != Some(side.other()) {
                    Some(format!(
                        "unit {target} is attacked, but it isn't an enemy alive"
                    ))
                } else {
                    None
                }
            }
            Event::DamageDealt { unit, .. } | Event::DivineShieldPopped { unit } => {
                alive(&side_of, &dead, *unit)
                    .is_none()
                    .then(|| format!("unit {unit} is hit, but it is unknown or dead"))
            }
            _ => None,
        };
        if let Some(problem) = problem {
            return Err(broken(
                format!("the battle's events don't add up: {}", normalize(&problem)),
                format!("event {i}: {problem}"),
            ));
        }
    }
    for (side, survivors, dead_units) in [
        (Side::A, &res.survivors_a, &res.dead_units_a),
        (Side::B, &res.survivors_b, &res.dead_units_b),
    ] {
        let expected: BTreeSet<UnitId> = side_of
            .iter()
            .filter(|(id, s)| **s == side && !dead.contains(id))
            .map(|(id, _)| *id)
            .collect();
        let got: BTreeSet<UnitId> = survivors.iter().map(|u| u.id).collect();
        if got != expected || got.len() != survivors.len() {
            return Err(broken(
                format!("the survivors of side {side:?} aren't the units that didn't die"),
                format!("survivors {got:?}, expected {expected:?}"),
            ));
        }
        // The dead units are the units of the side that died, once each.
        let died: BTreeSet<UnitId> = side_of
            .iter()
            .filter(|(id, s)| **s == side && dead.contains(id))
            .map(|(id, _)| *id)
            .collect();
        let listed: Vec<UnitId> = dead_units.iter().map(|u| u.id).collect();
        let listed_set: BTreeSet<UnitId> = listed.iter().copied().collect();
        if listed_set != died || listed.len() != died.len() {
            return Err(broken(
                format!("the dead units of side {side:?} aren't the units that died"),
                format!("dead_units {listed:?}, died {died:?}"),
            ));
        }
    }
    Ok(())
}

/// Every action a state could be asked to take, legal or not: each index up to one past its
/// zone's end, and a huge one.
fn candidate_actions(s: &TavernState) -> Vec<TavernAction> {
    let (b, h, sh) = (s.board.len(), s.hand.len(), s.shop.len());
    let opts = s.discover_pending.as_ref().map_or(0, |o| o.len());
    let far = usize::MAX;
    let mut out = Vec::new();
    for i in (0..=sh + 1).chain([far]) {
        out.push(TavernAction::Buy { shop_index: i });
    }
    for hand_index in (0..=h).chain([far]) {
        for board_pos in (0..=b + 1).chain([far]) {
            out.push(TavernAction::Play {
                hand_index,
                board_pos,
            });
        }
        for shop_index in (0..=sh).chain([far]) {
            out.push(TavernAction::PlayOnShop {
                hand_index,
                shop_index,
            });
        }
    }
    for board_pos in (0..=b).chain([far]) {
        out.push(TavernAction::Sell { board_pos });
        out.push(TavernAction::Activate {
            board_pos,
            target_pos: None,
        });
        for t in (0..=b.max(h).max(sh) + 1).chain([far]) {
            out.push(TavernAction::Activate {
                board_pos,
                target_pos: Some(t),
            });
        }
    }
    for from_pos in (0..=b).chain([far]) {
        for to_pos in (0..=b).chain([far]) {
            out.push(TavernAction::Reposition { from_pos, to_pos });
        }
    }
    for option_index in (0..=opts + 1).chain([far]) {
        out.push(TavernAction::ChooseDiscover { option_index });
    }
    for hand_index in (0..=h).chain([far]) {
        out.push(TavernAction::Pass { hand_index });
    }
    out.extend([
        TavernAction::Refresh,
        TavernAction::UpgradeTavern,
        TavernAction::ToggleFreeze,
        TavernAction::EndTurn,
    ]);
    out
}

/// `valid_actions()` lists each legal action once, `is_legal` agrees with it on every
/// candidate action, and a pending Discover allows only `ChooseDiscover` (`docs/tavern.md` §6).
/// Returns the actions `is_legal` accepts but `valid_actions` doesn't list.
fn check_legality(s: &TavernState, valid: &[TavernAction]) -> Result<Vec<TavernAction>, Violation> {
    if valid.is_empty() {
        return Err(broken("no action is valid", ""));
    }
    for (i, a) in valid.iter().enumerate() {
        if valid[..i].contains(a) {
            return Err(broken(format!("valid_actions lists {a:?} twice"), ""));
        }
        if !s.is_legal(a) {
            return Err(violation(
                FailureKind::LegalActionRejected,
                format!(
                    "valid_actions lists {:?} but is_legal rejects it",
                    kind_of(s, a)
                ),
                format!("{a:?}"),
            ));
        }
    }
    if let Some(opts) = &s.discover_pending {
        let expected: Vec<TavernAction> = (0..opts.len())
            .map(|option_index| TavernAction::ChooseDiscover { option_index })
            .collect();
        if valid != expected.as_slice() {
            return Err(broken(
                "a pending Discover allows other actions",
                format!("{valid:?}"),
            ));
        }
    }
    let mut unlisted = Vec::new();
    for a in candidate_actions(s) {
        if s.is_legal(&a) && !valid.contains(&a) {
            unlisted.push(a);
        }
    }
    Ok(unlisted)
}

/// The checks of one battle's result (`docs/combat.md` §3, §5.6, §6).
fn check_battle(res: &BattleResult, tier_a: u32, tier_b: u32) -> Result<(), Violation> {
    for (side, survivors) in [("A", &res.survivors_a), ("B", &res.survivors_b)] {
        if survivors.len() > 7 {
            return Err(broken(
                format!("side {side} has {} survivors", survivors.len()),
                units_desc(survivors),
            ));
        }
        if let Some(u) = survivors.iter().find(|u| u.health <= 0) {
            return Err(broken(
                format!("a dead minion survived: {}", u.name),
                units_desc(survivors),
            ));
        }
    }
    let (a, b) = (&res.survivors_a, &res.survivors_b);
    let can_attack = |units: &[Unit]| units.iter().any(|u| u.attack > 0);
    let consistent = match res.outcome {
        BattleOutcome::AWin => !a.is_empty() && b.is_empty(),
        BattleOutcome::BWin => a.is_empty() && !b.is_empty(),
        BattleOutcome::Draw => (a.is_empty() && b.is_empty()) || (!can_attack(a) && !can_attack(b)),
    };
    if !consistent {
        return Err(broken(
            format!("outcome {:?} with these survivors", res.outcome),
            format!("A {} B {}", units_desc(a), units_desc(b)),
        ));
    }
    let tiers = |units: &[Unit]| units.iter().map(|u| u.tavern_tier).sum::<u32>();
    let expected = match res.outcome {
        BattleOutcome::AWin => tier_a + tiers(a),
        BattleOutcome::BWin => tier_b + tiers(b),
        BattleOutcome::Draw => 0,
    };
    if res.hero_damage != expected || (res.hero_damage == 0) != (res.outcome == BattleOutcome::Draw)
    {
        return Err(broken(
            format!(
                "hero damage {} for {:?} (expected {expected})",
                res.hero_damage, res.outcome
            ),
            format!("A {} B {}", units_desc(a), units_desc(b)),
        ));
    }
    if !matches!(res.events.first(), Some(Event::BattleStart { .. })) {
        return Err(broken("the events don't start with BattleStart", ""));
    }
    match res.events.last() {
        Some(Event::BattleEnd {
            outcome,
            hero_damage,
        }) if *outcome == res.outcome && *hero_damage == res.hero_damage => Ok(()),
        other => Err(broken(
            "the events don't end with the BattleEnd of the result",
            format!("{other:?}"),
        )),
    }
}

/// The copies left in the pool of the catalog's minions of Tier `max_tier` or less.
fn pool_copies(pool: &CardPool, catalog: &[CardTemplate], max_tier: u32) -> u32 {
    catalog
        .iter()
        .filter(|t| t.tavern_tier <= max_tier)
        .map(|t| pool.remaining_copies(t.card_id))
        .sum()
}

/// The copies left in the pool of the catalog's minions of exactly Tier `tier`.
fn exact_tier_copies(pool: &CardPool, catalog: &[CardTemplate], tier: u32) -> u32 {
    catalog
        .iter()
        .filter(|t| t.tavern_tier == tier)
        .map(|t| pool.remaining_copies(t.card_id))
        .sum()
}

/// The pool never holds more than a card's base copies, and no copy leaves it for nowhere
/// (`docs/tavern.md` §2): each copy a card is missing from
/// the pool is a minion somewhere in the game (a board, a hand, a shop or a Discover; a Golden
/// formed by a Triple holds 3). Effects also make copies without taking them from the pool, so
/// only missing copies are checked, not extra ones.
fn check_pool_leaks(
    players: &[TavernState],
    pool: &CardPool,
    catalog: &[CardTemplate],
) -> Result<(), Violation> {
    let mut held: BTreeMap<u32, u32> = BTreeMap::new();
    for s in players {
        let prompts = s.discover_pending.iter().chain(&s.discover_queue).flatten();
        for u in s.board.iter().chain(&s.hand).chain(&s.shop).chain(prompts) {
            if !u.is_spell {
                let copies = if u.is_golden && !u.intrinsic_golden && !u.golden_from_effect {
                    3
                } else {
                    1
                };
                *held.entry(u.card_id).or_default() += copies;
            }
        }
    }
    for t in catalog {
        let remaining = pool.remaining_copies(t.card_id);
        let base = base_copies_for_tier(t.tavern_tier);
        if remaining > base {
            return Err(broken(
                format!(
                    "the pool has {remaining} copies of {} (base {base})",
                    t.name
                ),
                "",
            ));
        }
        if cards::hooks(t.card_id).has(CardFlags::TRIPLE_WILDCARD_ELEMENTAL) {
            continue; // it triples into another card's Golden
        }
        let out = base - remaining;
        let have = held.get(&t.card_id).copied().unwrap_or(0);
        if have < out {
            return Err(broken(
                "copies of a minion left the pool for nowhere",
                format!(
                    "{}: {out} copies out of the pool, {have} in the game",
                    t.name
                ),
            ));
        }
    }
    Ok(())
}

/// A shop just rolled (unfrozen `start_turn`, `Refresh`) holds `shop_capacity` minions from the
/// pool (fewer only if the pool has run out of them) and, with shop spells, one spell
/// (`docs/tavern.md` §4, §6).
fn check_refilled_shop(
    s: &TavernState,
    pool: &CardPool,
    catalog: &[CardTemplate],
    what: &str,
) -> Result<(), Violation> {
    let cap = shop_capacity(s.tavern_tier);
    let minions = s
        .shop
        .iter()
        .filter(|u| !u.is_spell && cards::template(u.card_id).is_some())
        .count();
    if minions < cap && pool_copies(pool, catalog, s.tavern_tier) > 0 {
        return Err(broken(
            format!("the shop rolled by {what} has {minions} pool minions (capacity {cap})"),
            units_desc(&s.shop),
        ));
    }
    // (Tokens an effect adds, such as a `Demon Fodder`, have a Tier of their own.)
    let rolled = |u: &&Unit| u.is_spell || cards::template(u.card_id).is_some();
    if let Some(u) = s
        .shop
        .iter()
        .filter(rolled)
        .find(|u| u.tavern_tier > s.tavern_tier)
    {
        return Err(broken(
            format!("the shop rolled by {what} offers a card above its Tier"),
            format!(
                "{} (Tier {}) at Tier {}",
                unit_desc(u),
                u.tavern_tier,
                s.tavern_tier
            ),
        ));
    }
    let spells = s.shop.iter().filter(|u| u.is_spell).count();
    let expected = usize::from(s.include_shop_spells);
    if spells != expected {
        return Err(broken(
            format!("the shop rolled by {what} has {spells} spells (expected {expected})"),
            units_desc(&s.shop),
        ));
    }
    Ok(())
}

fn is_wildcard(u: &Unit) -> bool {
    cards::hooks(u.card_id).has(CardFlags::TRIPLE_WILDCARD_ELEMENTAL)
}

/// A combat changes none of the Tavern's minions but by what they keep from it
/// (`docs/combat.md` §1, Combat-to-Tavern Persistence): the board keeps its minions in order,
/// the shop its cards, the hand its cards (new ones come after them, and a Triple can form), and
/// the Gold, maximum Gold, Tier and turn don't change.
fn check_combat_keeps_tavern(before: &TavernState, after: &TavernState) -> Result<(), Violation> {
    let ids = |units: &[Unit]| units.iter().map(|u| u.card_id).collect::<Vec<_>>();
    // A card the combat put in the hand can complete a Triple with copies on the board
    // (`docs/tavern.md` §5): those copies leave the board for the Golden in the hand.
    let golden = |s: &TavernState| s.hand.iter().filter(|u| u.is_golden).count();
    let tripled: Vec<u32> = if golden(after) > golden(before) {
        after
            .hand
            .iter()
            .filter(|u| u.is_golden)
            .map(|u| u.card_id)
            .collect()
    } else {
        Vec::new()
    };
    let board_kept = |b: &[Unit], a: &[Unit]| {
        let mut rest = a.iter().peekable();
        b.iter().all(|u| {
            if rest.peek().is_some_and(|x| x.card_id == u.card_id) {
                rest.next();
                true
            } else {
                tripled.contains(&u.card_id) || (!tripled.is_empty() && is_wildcard(u))
            }
        }) && rest.next().is_none()
    };
    let problem = if !board_kept(&before.board, &after.board) {
        Some((
            "the board's minions changed",
            format!(
                "{} -> {}",
                units_desc(&before.board),
                units_desc(&after.board)
            ),
        ))
    } else if ids(&before.shop) != ids(&after.shop) {
        Some((
            "the shop's cards changed",
            format!(
                "{} -> {}",
                units_desc(&before.shop),
                units_desc(&after.shop)
            ),
        ))
    } else if (
        before.gold,
        before.max_gold,
        before.tavern_tier,
        before.turn,
    ) != (after.gold, after.max_gold, after.tavern_tier, after.turn)
    {
        Some((
            "the Gold, Tier or turn changed",
            format!(
                "gold {}/{} tier {} turn {} -> gold {}/{} tier {} turn {}",
                before.gold,
                before.max_gold,
                before.tavern_tier,
                before.turn,
                after.gold,
                after.max_gold,
                after.tavern_tier,
                after.turn
            ),
        ))
    } else {
        let tripled = !tripled.is_empty();
        let lockbox = |u: &Unit| u.unplayable || u.lockbox_turns_left > 0;
        let kept = before.hand.len() <= after.hand.len()
            && before
                .hand
                .iter()
                .zip(&after.hand)
                .all(|(b, a)| b.card_id == a.card_id || lockbox(b));
        (!tripled && !kept).then(|| {
            (
                "the hand lost cards",
                format!(
                    "{} -> {}",
                    units_desc(&before.hand),
                    units_desc(&after.hand)
                ),
            )
        })
    };
    match problem {
        Some((what, detail)) => Err(broken(
            format!("a combat changed the Tavern: {what}"),
            detail,
        )),
        None => Ok(()),
    }
}

/// A Tavern state survives a serde round trip unchanged (snapshots, the Python bindings and the
/// scenario DSL rely on its serde form holding the whole state).
fn check_round_trip(s: &TavernState) -> Result<(), Violation> {
    let text = yaml(s);
    match serde_yaml::from_str::<TavernState>(&text) {
        Ok(back) if format!("{back:?}") == format!("{s:?}") => Ok(()),
        Ok(back) => Err(broken(
            "a Tavern state changes in a serde round trip",
            field_changes(s, &back),
        )),
        Err(e) => Err(broken(
            "a Tavern state doesn't deserialize from its serde form",
            e.to_string(),
        )),
    }
}

/// Nothing heals the hero: its Health never goes up (`Soul Rewinder` prevents damage instead).
fn check_health_kept(before: i32, s: &TavernState) -> Result<(), Violation> {
    if s.health > before {
        return Err(broken(
            "the hero's Health went up",
            format!("{before} -> {}", s.health),
        ));
    }
    Ok(())
}

/// A hero's Health and Armor together.
fn hit_points(s: &TavernState) -> i32 {
    s.health + s.armor
}

// ---------------------------------------------------------------------------------------------
// Snapshots and digests
// ---------------------------------------------------------------------------------------------

fn yaml<T: Serialize>(value: &T) -> String {
    serde_yaml::to_string(value).unwrap_or_else(|e| format!("<not serializable: {e}>"))
}

fn rng_state(rng: &Rng) -> u64 {
    // `Rng::new(state)` restarts a generator from its state.
    let text = format!("{rng:?}");
    text.chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .expect("Rng's Debug shows its state")
}

/// The copies of each card the pool is missing (base copies less the remaining ones).
fn pool_taken(pool: &CardPool, catalog: &[CardTemplate]) -> BTreeMap<String, u32> {
    catalog
        .iter()
        .filter_map(|t| {
            let taken = base_copies_for_tier(t.tavern_tier)
                .saturating_sub(pool.remaining_copies(t.card_id));
            (taken > 0).then(|| (t.name.clone(), taken))
        })
        .collect()
}

fn digest(players: &[TavernState], pool: &CardPool, rng: &Rng, catalog: &[CardTemplate]) -> u64 {
    let mut h = DefaultHasher::new();
    for p in players {
        format!("{p:?}").hash(&mut h);
    }
    for t in catalog {
        pool.remaining_copies(t.card_id).hash(&mut h);
    }
    rng_state(rng).hash(&mut h);
    h.finish()
}

/// A serializable `TavernAction`, for snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Act {
    Buy(usize),
    Play(usize, usize),
    PlayOnShop(usize, usize),
    Sell(usize),
    Reposition(usize, usize),
    Activate(usize, Option<usize>),
    Refresh,
    UpgradeTavern,
    ToggleFreeze,
    ChooseDiscover(usize),
    EndTurn,
    Pass(usize),
}

impl From<TavernAction> for Act {
    fn from(a: TavernAction) -> Self {
        match a {
            TavernAction::Buy { shop_index } => Act::Buy(shop_index),
            TavernAction::Play {
                hand_index,
                board_pos,
            } => Act::Play(hand_index, board_pos),
            TavernAction::PlayOnShop {
                hand_index,
                shop_index,
            } => Act::PlayOnShop(hand_index, shop_index),
            TavernAction::Sell { board_pos } => Act::Sell(board_pos),
            TavernAction::Reposition { from_pos, to_pos } => Act::Reposition(from_pos, to_pos),
            TavernAction::Activate {
                board_pos,
                target_pos,
            } => Act::Activate(board_pos, target_pos),
            TavernAction::Refresh => Act::Refresh,
            TavernAction::UpgradeTavern => Act::UpgradeTavern,
            TavernAction::ToggleFreeze => Act::ToggleFreeze,
            TavernAction::ChooseDiscover { option_index } => Act::ChooseDiscover(option_index),
            TavernAction::EndTurn => Act::EndTurn,
            TavernAction::Pass { hand_index } => Act::Pass(hand_index),
        }
    }
}

impl From<Act> for TavernAction {
    fn from(a: Act) -> Self {
        match a {
            Act::Buy(shop_index) => TavernAction::Buy { shop_index },
            Act::Play(hand_index, board_pos) => TavernAction::Play {
                hand_index,
                board_pos,
            },
            Act::PlayOnShop(hand_index, shop_index) => TavernAction::PlayOnShop {
                hand_index,
                shop_index,
            },
            Act::Sell(board_pos) => TavernAction::Sell { board_pos },
            Act::Reposition(from_pos, to_pos) => TavernAction::Reposition { from_pos, to_pos },
            Act::Activate(board_pos, target_pos) => TavernAction::Activate {
                board_pos,
                target_pos,
            },
            Act::Refresh => TavernAction::Refresh,
            Act::UpgradeTavern => TavernAction::UpgradeTavern,
            Act::ToggleFreeze => TavernAction::ToggleFreeze,
            Act::ChooseDiscover(option_index) => TavernAction::ChooseDiscover { option_index },
            Act::EndTurn => TavernAction::EndTurn,
            Act::Pass(hand_index) => TavernAction::Pass { hand_index },
        }
    }
}

/// Everything needed to replay one failing engine call: the game's state just before it.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Snapshot {
    seed: u64,
    call: u64,
    turn: u32,
    phase: Phase,
    kind: FailureKind,
    signature: String,
    detail: String,
    cards: String,
    catalog: String,
    /// The engine RNG's state (`Rng::new(rng_state)`).
    rng_state: u64,
    /// The copies of each card the pool is missing.
    pool_taken: BTreeMap<String, u32>,
    action: Option<Act>,
    /// The combat seed, for a combat.
    combat_seed: Option<u64>,
    player: usize,
    state: TavernState,
    /// The opponent of a combat (or the ghost's state).
    opponent: Option<TavernState>,
}

// ---------------------------------------------------------------------------------------------
// One game
// ---------------------------------------------------------------------------------------------

/// Why a game stopped early.
enum Halt {
    Failed(Violation),
    /// It reached the call to snapshot (`Game::stop_at`).
    Snapshot,
}

impl From<Violation> for Halt {
    fn from(v: Violation) -> Self {
        Halt::Failed(v)
    }
}

type Flow = Result<(), Halt>;

/// The inputs of the engine call a game stopped at (`Game::stop_at`).
struct Taken {
    call: Call,
    players: Vec<TavernState>,
    pool: CardPool,
    rng: Rng,
    ghost: Option<TavernState>,
    combat_seed: Option<u64>,
}

struct Game {
    seed: u64,
    cfg: Config,
    catalog: Vec<CardTemplate>,
    pool: CardPool,
    rng: Rng,
    /// The fuzzer's own choices (never the engine's).
    policy: Rng,
    players: Vec<TavernState>,
    alive: Vec<bool>,
    /// The last eliminated player, whose ghost the odd player out fights.
    ghost: Option<TavernState>,
    call: Call,
    /// Stop just before this engine call, keeping its inputs in `taken`.
    stop_at: Option<u64>,
    taken: Option<Taken>,
    /// The ghost the current combat is against.
    ghost_now: Option<TavernState>,
    digests: Vec<u64>,
    lookahead: u64,
    trace: bool,
    /// The cards involved in the current call.
    cards: String,
    /// How many times each card was played from hand (coverage).
    played: BTreeMap<u32, u32>,
    /// Go on past the failures of known bugs ([`KNOWN_BUGS`]), keeping them in `known`.
    continue_known: bool,
    known: Vec<Failure>,
}

impl Game {
    fn new(seed: u64, settings: &Settings) -> Self {
        let cfg = Config::random(seed, settings.turns);
        let catalog = catalog_for(cfg.catalog).expect("known catalog");
        let pool = CardPool::new(catalog.clone());
        let mut players: Vec<TavernState> = (0..cfg.players)
            .map(|i| {
                let mut p = TavernState::new().with_shop_spells(cfg.shop_spells);
                p.armor = cfg.armor[i];
                p.auras.deity.kind = cfg.deities[i];
                p.auras.base_max_gold_bonus = cfg.rich;
                p
            })
            .collect();
        for (player, card) in &cfg.gifts {
            players[*player].add_to_hand(card.clone());
        }
        Self {
            seed,
            alive: vec![true; cfg.players],
            cfg,
            catalog,
            pool,
            rng: Rng::new(seed),
            policy: Rng::new(seed ^ 0x9071_C700_0000_0001),
            players,
            ghost: None,
            call: Call {
                index: 0,
                turn: 0,
                phase: Phase::StartTurn,
                player: 0,
                opponent: None,
                action: None,
            },
            stop_at: None,
            taken: None,
            ghost_now: None,
            digests: Vec::new(),
            lookahead: settings.lookahead,
            trace: false,
            cards: String::new(),
            played: BTreeMap::new(),
            continue_known: settings.continue_known,
            known: Vec::new(),
        }
    }

    /// The failure of `v` at the current call.
    fn failure(&self, v: Violation) -> Failure {
        Failure {
            seed: self.seed,
            kind: v.kind,
            signature: format!("{:?}: {}", v.kind, normalize(&v.what)),
            detail: format!("{}: {}", v.what, v.detail),
            call: self.call.clone(),
            cards: self.cards.clone(),
        }
    }

    /// Stop at the first failed check, unless it is a known bug and the game goes on past
    /// those (`continue_known`): then keep it in `known` (once per game) and go on.
    fn tolerate<const N: usize>(&mut self, checks: [Result<(), Violation>; N]) -> Flow {
        for check in checks {
            if let Err(v) = check {
                let failure = self.failure(v.clone());
                if !(self.continue_known && known_bug(&failure).is_some()) {
                    return Err(Halt::Failed(v));
                }
                if !self.known.iter().any(|f| f.signature == failure.signature) {
                    self.known.push(failure);
                }
            }
        }
        Ok(())
    }

    /// Start the next engine call; stop here if it is the one to snapshot.
    fn begin(
        &mut self,
        phase: Phase,
        player: usize,
        opponent: Option<usize>,
        combat_seed: Option<u64>,
    ) -> Flow {
        self.call.index += 1;
        self.call.phase = phase;
        self.call.player = player;
        self.call.opponent = opponent;
        self.call.action = None;
        self.cards.clear();
        if self.stop_at == Some(self.call.index) {
            self.taken = Some(Taken {
                call: self.call.clone(),
                players: self.players.clone(),
                pool: self.pool.clone(),
                rng: self.rng.clone(),
                ghost: self.ghost_now.clone().filter(|_| phase == Phase::Ghost),
                combat_seed,
            });
            return Err(Halt::Snapshot);
        }
        Ok(())
    }

    fn record_digest(&mut self) {
        let d = digest(&self.players, &self.pool, &self.rng, &self.catalog);
        self.digests.push(d);
    }

    fn play(&mut self) -> Flow {
        for turn in 1..=self.cfg.max_turns {
            self.call.turn = turn;
            for p in 0..self.players.len() {
                if !self.alive[p] {
                    continue;
                }
                self.begin(Phase::StartTurn, p, None, None)?;
                let (was_frozen, old_cost, old_health) = (
                    self.players[p].is_frozen,
                    self.players[p].upgrade_cost,
                    self.players[p].health,
                );
                self.players[p].start_turn(&mut self.pool, &mut self.rng);
                if self.trace {
                    let s = &self.players[p];
                    println!(
                        "  t{turn} p{p}: {} hp {} armor, tier {}, {} gold\n    board {}\n    hand {}\n    shop {}",
                        s.health,
                        s.armor,
                        s.tavern_tier,
                        s.gold,
                        units_desc(&s.board),
                        units_desc(&s.hand),
                        units_desc(&s.shop)
                    );
                }
                let s = &self.players[p];
                if s.is_frozen {
                    return Err(broken("the shop is still frozen after start_turn", "").into());
                }
                if s.turn > 1 && s.tavern_tier < 6 && s.upgrade_cost != old_cost.saturating_sub(1) {
                    return Err(broken(
                        "the upgrade cost did not go down by 1 at the start of the turn",
                        format!("{old_cost} -> {}", s.upgrade_cost),
                    )
                    .into());
                }
                let refilled = if was_frozen {
                    Ok(())
                } else {
                    check_refilled_shop(s, &self.pool, &self.catalog, "start_turn")
                };
                let checks = [
                    refilled,
                    check_health_kept(old_health, s),
                    check_synced(s),
                    check_tavern(s),
                    check_pool_leaks(&self.players, &self.pool, &self.catalog),
                ];
                self.tolerate(checks)?;
            }
            for p in 0..self.players.len() {
                if self.alive[p] {
                    self.tavern_turn(p)?;
                    let checks = [
                        check_pool_leaks(&self.players, &self.pool, &self.catalog),
                        check_round_trip(&self.players[p]),
                    ];
                    self.tolerate(checks)?;
                }
            }
            self.combats()?;
            self.record_digest();
            if self.alive.iter().filter(|&&a| a).count() <= 1 {
                break;
            }
        }
        Ok(())
    }

    /// One player's Tavern turn: random actions until `EndTurn` or the action cap.
    fn tavern_turn(&mut self, p: usize) -> Flow {
        let mut steps = 0;
        let mut discovers_in_a_row = 0;
        loop {
            self.begin(Phase::Action, p, None, None)?;
            let s = &self.players[p];
            let valid = s.valid_actions();
            let unlisted = check_legality(s, &valid)?;
            if let Some(&a) = unlisted.first() {
                // Step it first: a crash says more than the disagreement.
                self.step_on_copy(p, a, Phase::Unlisted)?;
                let s = &self.players[p];
                self.call.action = Some(a);
                self.cards = Self::involved(s, &a);
                return Err(broken(
                    format!(
                        "is_legal accepts a {:?} that valid_actions doesn't list",
                        kind_of(s, &a)
                    ),
                    format!("{a:?}"),
                )
                .into());
            }
            if self.policy.below(1000) < self.lookahead as usize {
                for &a in &valid {
                    self.step_on_copy(p, a, Phase::Lookahead)?;
                }
            }
            let s = &self.players[p];
            let action = if s.discover_pending.is_some() {
                discovers_in_a_row += 1;
                if discovers_in_a_row > 50 {
                    return Err(broken("more than 50 Discovers in a row", "").into());
                }
                valid[self.policy.below(valid.len())]
            } else if steps >= self.cfg.action_cap {
                TavernAction::EndTurn
            } else {
                discovers_in_a_row = 0;
                choose(s, &valid, &self.cfg.weights, steps, &mut self.policy)
            };
            steps += 1;
            let ended = self.step(p, action, Phase::Action)?;
            if ended {
                return Ok(());
            }
        }
    }

    /// Describe the cards `action` involves in `s`.
    fn involved(s: &TavernState, action: &TavernAction) -> String {
        let card = |units: &[Unit], i: usize| units.get(i).map(unit_desc).unwrap_or_default();
        match *action {
            TavernAction::Buy { shop_index } => card(&s.shop, shop_index),
            TavernAction::Play {
                hand_index,
                board_pos,
            } => format!(
                "{} -> board[{board_pos}] {}",
                card(&s.hand, hand_index),
                card(&s.board, board_pos)
            ),
            TavernAction::PlayOnShop {
                hand_index,
                shop_index,
            } => format!(
                "{} -> shop[{shop_index}] {}",
                card(&s.hand, hand_index),
                card(&s.shop, shop_index)
            ),
            TavernAction::Sell { board_pos } => card(&s.board, board_pos),
            TavernAction::Activate {
                board_pos,
                target_pos,
            } => format!("{} -> {target_pos:?}", card(&s.board, board_pos)),
            TavernAction::ChooseDiscover { option_index } => s
                .discover_pending
                .as_ref()
                .map(|o| card(o, option_index))
                .unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// Step `action` for player `p`, with the checks of the result.
    fn step(&mut self, p: usize, action: TavernAction, phase: Phase) -> Result<bool, Halt> {
        self.call.phase = phase;
        self.call.action = Some(action);
        self.cards = Self::involved(&self.players[p], &action);
        if self.trace {
            println!("    t{} p{p} {action:?} {}", self.call.turn, self.cards);
        }
        let s = &mut self.players[p];
        let kind = kind_of(s, &action);
        let before_hp = hit_points(s);
        let old_health = s.health;
        let health_cost = match action {
            TavernAction::Refresh => s.auras.free_refreshes == 0 && s.has_health_refresh(),
            TavernAction::Buy { shop_index } => s
                .shop
                .get(shop_index)
                .is_some_and(|u| u.is_spell && u.costs_health),
            _ => false,
        };
        let tier = s.tavern_tier;
        let prompts = s.discover_pending.iter().chain(&s.discover_queue).count();
        let tavern_spells_cast = s.auras.tavern_spells_cast;
        // The card an action puts in the hand (a bought minion, a Discovered card), and the
        // Golden played from hand that must offer a Triple Reward.
        let mut to_hand = None;
        let mut triple_reward = false;
        let reward_tier = (s.tavern_tier + 1).min(6);
        let mut reward_tier_left = false;
        let mut casts_tavern_spell = false;
        let room = s.hand.len() < 10;
        let gold = s.gold;
        let free_refresh = action == TavernAction::Refresh
            && (s.auras.free_refreshes > 0 || s.has_health_refresh());
        // What the action costs in Gold (`docs/tavern.md` §6), when nothing else changes the Gold.
        let cost = match action {
            TavernAction::Buy { shop_index } => s.shop.get(shop_index).map(|u| {
                if !u.is_spell {
                    3
                } else if u.costs_health {
                    0
                } else {
                    s.effective_spell_buy_cost(u)
                }
            }),
            TavernAction::Refresh => Some(if free_refresh { 0 } else { 1 }),
            TavernAction::UpgradeTavern => Some(s.upgrade_cost),
            // (Not an Activate: its effect can give Gold, a Kelp Keeper triggering a Battlecry
            // that casts a Tavern Coin.)
            _ => None,
        };
        if let TavernAction::Play { hand_index, .. } | TavernAction::PlayOnShop { hand_index, .. } =
            action
        {
            if let Some(card) = s.hand.get(hand_index) {
                *self.played.entry(card.card_id).or_default() += 1;
            }
        }
        match action {
            TavernAction::Buy { shop_index } => {
                // (A wildcard `Elemental of Surprise` triples into another card's Golden.)
                to_hand = s
                    .shop
                    .get(shop_index)
                    .filter(|u| !u.is_spell && !is_wildcard(u))
                    .map(|u| u.card_id);
            }
            TavernAction::ChooseDiscover { option_index } => {
                to_hand = s
                    .discover_pending
                    .as_ref()
                    .and_then(|o| o.get(option_index))
                    .filter(|u| !u.is_spell && !is_wildcard(u) && !cards::is_option_card(u.card_id))
                    .map(|u| u.card_id)
                    .filter(|_| room);
            }
            TavernAction::Play { hand_index, .. } | TavernAction::PlayOnShop { hand_index, .. } => {
                if let Some(card) = s.hand.get(hand_index) {
                    triple_reward = !card.is_spell
                        && card.is_golden
                        && !card.intrinsic_golden
                        && pool_copies(&self.pool, &self.catalog, reward_tier) >= 3;
                    reward_tier_left = triple_reward
                        && exact_tier_copies(&self.pool, &self.catalog, reward_tier) > 0;
                    casts_tavern_spell =
                        card.is_spell && cards::spells::is_tavern_spell(card.card_id);
                }
            }
            _ => {}
        }
        let ended = match s.step(action, &mut self.pool, &mut self.rng) {
            Ok(ended) => ended,
            Err(e) => {
                return Err(violation(
                    FailureKind::LegalActionRejected,
                    format!("step rejects a listed {:?}", kind),
                    e,
                )
                .into())
            }
        };
        if ended != (action == TavernAction::EndTurn) {
            return Err(broken(format!("step returned {ended} for {action:?}"), "").into());
        }
        let s = &self.players[p];
        if health_cost && before_hp > 0 && s.health <= 0 {
            return Err(violation(
                FailureKind::IllegalActionAccepted,
                format!("a Health cost ({:?}) killed the hero", kind),
                format!("{before_hp} -> {} Health + Armor", hit_points(s)),
            )
            .into());
        }
        if action == TavernAction::UpgradeTavern
            && (s.tavern_tier != tier + 1 || s.upgrade_cost != base_upgrade_cost(s.tavern_tier))
        {
            return Err(broken(
                "UpgradeTavern did not go up one Tier at its base upgrade cost",
                format!("tier {tier} -> {}, cost {}", s.tavern_tier, s.upgrade_cost),
            )
            .into());
        }
        if let Some(cid) = to_hand {
            if !s.hand.iter().any(|u| u.card_id == cid) {
                let lost = broken(
                    format!("the {:?} card did not reach the hand", kind),
                    format!("card id {cid}, hand {}", units_desc(&s.hand)),
                );
                self.tolerate([Err(lost)])?;
            }
        }
        let s = &self.players[p];
        if triple_reward {
            let last = s.discover_queue.last().or(s.discover_pending.as_ref());
            if s.discover_pending.iter().chain(&s.discover_queue).count() <= prompts {
                return Err(broken(
                    "a Golden played from hand offered no Triple Reward",
                    format!("board {}", units_desc(&s.board)),
                )
                .into());
            }
            if let Some(opts) = last.filter(|_| reward_tier_left) {
                if opts.iter().any(|u| u.tavern_tier != reward_tier) {
                    return Err(broken(
                        "a Triple Reward offers minions of another Tier",
                        format!("Tier {reward_tier} expected: {}", units_desc(opts)),
                    )
                    .into());
                }
            }
        }
        if let Some(cost) = cost {
            if s.gold + cost != gold {
                return Err(broken(
                    format!("a {:?} did not cost its price in Gold", kind),
                    format!("{gold} -> {} Gold for a cost of {cost}", s.gold),
                )
                .into());
            }
        }
        if matches!(action, TavernAction::Sell { .. }) && s.gold < gold + 1 {
            return Err(broken(
                "a Sell did not give 1 Gold",
                format!("{gold} -> {} Gold", s.gold),
            )
            .into());
        }
        if casts_tavern_spell && s.auras.tavern_spells_cast <= tavern_spells_cast {
            return Err(broken(
                "a Tavern spell played from hand was not counted as cast",
                format!("tavern_spells_cast {}", s.auras.tavern_spells_cast),
            )
            .into());
        }
        if action == TavernAction::Refresh {
            if s.is_frozen {
                return Err(broken("the shop is still frozen after a Refresh", "").into());
            }
            check_refilled_shop(s, &self.pool, &self.catalog, "Refresh")?;
        }
        let leaks = if self.trace {
            check_pool_leaks(&self.players, &self.pool, &self.catalog)
        } else {
            Ok(())
        };
        let checks = [
            check_health_kept(old_health, s),
            check_synced(s),
            check_tavern(s),
            leaks,
        ];
        self.tolerate(checks)?;
        Ok(ended)
    }

    /// Step `action` for player `p` on a copy of the game, with the checks of the result.
    fn step_on_copy(&mut self, p: usize, action: TavernAction, phase: Phase) -> Flow {
        let (player, pool, rng) = (self.players[p].clone(), self.pool.clone(), self.rng.clone());
        let trace = std::mem::replace(&mut self.trace, false);
        let result = self.step(p, action, phase);
        self.trace = trace;
        self.players[p] = player;
        self.pool = pool;
        self.rng = rng;
        self.call.action = None;
        result.map(|_| ())
    }

    /// Pair the players still in the game and resolve their combats.
    fn combats(&mut self) -> Flow {
        let mut order: Vec<usize> = (0..self.players.len()).filter(|&p| self.alive[p]).collect();
        for i in (1..order.len()).rev() {
            let j = self.policy.below(i + 1);
            order.swap(i, j);
        }
        for pair in order.chunks(2) {
            if let [a, b] = *pair {
                self.combat_pair(a, b)?;
            } else {
                self.combat_ghost(pair[0])?;
            }
            let leaks = check_pool_leaks(&self.players, &self.pool, &self.catalog);
            self.tolerate([leaks])?;
        }
        for p in 0..self.players.len() {
            if self.alive[p] && self.players[p].health <= 0 {
                self.alive[p] = false;
                self.ghost = Some(self.players[p].clone());
            }
        }
        Ok(())
    }

    fn combat_pair(&mut self, a: usize, b: usize) -> Flow {
        let seed = self.rng.next_u64();
        self.begin(Phase::Combat, a, Some(b), Some(seed))?;
        let (tier_a, tier_b) = (self.players[a].tavern_tier, self.players[b].tavern_tier);
        let (hp_a, hp_b) = (hit_points(&self.players[a]), hit_points(&self.players[b]));
        let (health_a, health_b) = (self.players[a].health, self.players[b].health);
        let len_a = self.players[a].board.len().min(7);
        let len_b = self.players[b].board.len().min(7);
        let (before_a, before_b) = (self.players[a].clone(), self.players[b].clone());
        let (mut pa, mut pb) = (self.players[a].clone(), self.players[b].clone());
        let res = TavernState::resolve_combat_pair(&mut pa, &mut pb, seed);
        self.players[a] = pa;
        self.players[b] = pb;
        if self.trace {
            println!(
                "  combat p{a} v p{b}: {:?}, {} damage",
                res.outcome, res.hero_damage
            );
        }
        check_battle(&res, tier_a, tier_b)?;
        check_battle_units(&res, len_a, len_b)?;
        let (loss_a, loss_b) = match res.outcome {
            BattleOutcome::AWin => (0, res.hero_damage as i32),
            BattleOutcome::BWin => (res.hero_damage as i32, 0),
            BattleOutcome::Draw => (0, 0),
        };
        let checks = [
            check_combat_keeps_tavern(&before_a, &self.players[a]),
            check_combat_keeps_tavern(&before_b, &self.players[b]),
        ];
        self.tolerate(checks)?;
        for (p, before, loss, health) in [(a, hp_a, loss_a, health_a), (b, hp_b, loss_b, health_b)]
        {
            let checks = [
                check_health_kept(health, &self.players[p]),
                check_synced(&self.players[p]),
            ];
            self.tolerate(checks)?;
            let after = hit_points(&self.players[p]);
            if after != before - loss {
                return Err(broken(
                    "the hero lost other than the combat's damage",
                    format!(
                        "player {p}: {before} -> {after} Health + Armor, expected -{loss} ({:?})",
                        res.outcome
                    ),
                )
                .into());
            }
            let checks = [check_tavern(&self.players[p])];
            self.tolerate(checks)?;
        }
        Ok(())
    }

    /// The odd player out fights the last eliminated player's ghost (or, before anyone is
    /// eliminated, a copy of a random other player's warband).
    fn combat_ghost(&mut self, p: usize) -> Flow {
        let others: Vec<usize> = (0..self.players.len())
            .filter(|&q| q != p && self.alive[q])
            .collect();
        let opponent = match &self.ghost {
            Some(g) => g.clone(),
            None if !others.is_empty() => {
                self.players[others[self.policy.below(others.len())]].clone()
            }
            None => return Ok(()),
        };
        self.ghost_now = Some(opponent.clone());
        let seed = self.rng.next_u64();
        self.begin(Phase::Ghost, p, None, Some(seed))?;
        let tier = self.players[p].tavern_tier;
        let hp = hit_points(&self.players[p]);
        let health = self.players[p].health;
        let (len_a, len_b) = (
            self.players[p].board.len().min(7),
            opponent.board.len().min(7),
        );
        let before = self.players[p].clone();
        let res = self.players[p].resolve_combat_against(
            &opponent.board,
            opponent.tavern_tier,
            &opponent.auras,
            &opponent.hand,
            seed,
        );
        if self.trace {
            println!(
                "  combat p{p} v ghost: {:?}, {} damage",
                res.outcome, res.hero_damage
            );
        }
        check_battle(&res, tier, opponent.tavern_tier)?;
        check_battle_units(&res, len_a, len_b)?;
        let checks = [
            check_combat_keeps_tavern(&before, &self.players[p]),
            check_health_kept(health, &self.players[p]),
            check_synced(&self.players[p]),
        ];
        self.tolerate(checks)?;
        let loss = if res.outcome == BattleOutcome::BWin {
            res.hero_damage as i32
        } else {
            0
        };
        let after = hit_points(&self.players[p]);
        if after != hp - loss {
            return Err(broken(
                "the hero lost other than the combat's damage",
                format!(
                    "player {p}: {hp} -> {after} Health + Armor, expected -{loss} ({:?})",
                    res.outcome
                ),
            )
            .into());
        }
        let checks = [check_tavern(&self.players[p])];
        self.tolerate(checks)?;
        Ok(())
    }
}

/// The outcome of one run of a game: its digests, and its failure if it failed.
struct Run {
    digests: Vec<u64>,
    failure: Option<Failure>,
    /// The failures of known bugs the game went on past.
    known: Vec<Failure>,
    played: BTreeMap<u32, u32>,
}

/// Play the game of `seed` once, catching its panics.
fn run_game(seed: u64, settings: &Settings, trace: bool) -> Run {
    install_panic_hook();
    let mut game = Game::new(seed, settings);
    game.trace = trace;
    if trace {
        println!("game {seed}: {:?}", game.cfg);
    }
    CAPTURING.with(|c| *c.borrow_mut() = true);
    let result = panic::catch_unwind(AssertUnwindSafe(|| game.play()));
    CAPTURING.with(|c| *c.borrow_mut() = false);
    let failure = match result {
        Ok(Ok(())) | Ok(Err(Halt::Snapshot)) => None,
        Ok(Err(Halt::Failed(v))) => Some(Failure {
            seed,
            kind: v.kind,
            signature: format!("{:?}: {}", v.kind, normalize(&v.what)),
            detail: format!("{}: {}", v.what, v.detail),
            call: game.call.clone(),
            cards: game.cards.clone(),
        }),
        Err(_) => {
            let message = LAST_PANIC
                .with(|p| p.borrow_mut().take())
                .unwrap_or_else(|| "<panic>".to_string());
            let first_line = message.lines().next().unwrap_or_default().to_string();
            Some(Failure {
                seed,
                kind: FailureKind::Crash,
                signature: format!("Crash: {}", normalize(&first_line)),
                detail: message,
                call: game.call.clone(),
                cards: game.cards.clone(),
            })
        }
    };
    Run {
        digests: game.digests,
        failure,
        known: game.known,
        played: game.played,
    }
}

/// Play the game of `seed` twice: the failure of the first run, or a nondeterminism if the
/// replay differs.
fn fuzz_game(seed: u64, settings: &Settings) -> (Vec<Failure>, BTreeMap<u32, u32>) {
    let first = run_game(seed, settings, false);
    let played = first.played.clone();
    let mut failures = first.known.clone();
    failures.extend(check_replay(seed, settings, first));
    (failures, played)
}

/// The failure of the first run of a game, or a nondeterminism if its replay differs.
fn check_replay(seed: u64, settings: &Settings, first: Run) -> Option<Failure> {
    let replay = run_game(seed, settings, false);
    let same =
        |f: &Failure, g: &Failure| f.signature == g.signature && f.call.index == g.call.index;
    let same_failure = match (&first.failure, &replay.failure) {
        (None, None) => true,
        (Some(f), Some(g)) => same(f, g),
        _ => false,
    } && first.known.len() == replay.known.len()
        && first
            .known
            .iter()
            .zip(&replay.known)
            .all(|(f, g)| same(f, g));
    if first.digests != replay.digests || !same_failure {
        let at = first
            .digests
            .iter()
            .zip(&replay.digests)
            .position(|(x, y)| x != y);
        let call = first
            .failure
            .as_ref()
            .map(|f| f.call.clone())
            .unwrap_or(Call {
                index: 0,
                turn: 0,
                phase: Phase::StartTurn,
                player: 0,
                opponent: None,
                action: None,
            });
        return Some(Failure {
            seed,
            kind: FailureKind::Nondeterminism,
            signature: "Nondeterminism: the replay of a game differs".to_string(),
            detail: format!(
                "digests differ at {at:?} of {}/{}; failures {:?} / {:?}",
                first.digests.len(),
                replay.digests.len(),
                first.failure.as_ref().map(|f| &f.signature),
                replay.failure.as_ref().map(|f| &f.signature)
            ),
            call,
            cards: String::new(),
        });
    }
    first.failure
}

/// Replay the game of `failure` up to its failing call and snapshot the state just before it.
fn snapshot(failure: &Failure, settings: &Settings) -> Option<Snapshot> {
    let mut game = Game::new(failure.seed, settings);
    game.stop_at = Some(failure.call.index);
    CAPTURING.with(|c| *c.borrow_mut() = true);
    let _ = panic::catch_unwind(AssertUnwindSafe(|| game.play()));
    CAPTURING.with(|c| *c.borrow_mut() = false);
    let Taken {
        call,
        players,
        pool,
        rng,
        ghost,
        combat_seed,
    } = game.taken.take()?;
    let opponent = call.opponent.map(|o| players[o].clone()).or(ghost);
    Some(Snapshot {
        seed: failure.seed,
        call: call.index,
        turn: call.turn,
        phase: failure.call.phase,
        kind: failure.kind,
        signature: failure.signature.clone(),
        detail: failure.detail.clone(),
        cards: failure.cards.clone(),
        catalog: game.cfg.catalog.to_string(),
        rng_state: rng_state(&rng),
        pool_taken: pool_taken(&pool, &game.catalog),
        action: failure.call.action.map(Act::from),
        combat_seed,
        player: call.player,
        state: players[call.player].clone(),
        opponent,
    })
}

/// Rebuild a snapshot's pool.
fn snapshot_pool(snap: &Snapshot) -> (CardPool, Vec<CardTemplate>) {
    let catalog = catalog_for(&snap.catalog).expect("known catalog");
    let mut pool = CardPool::new(catalog.clone());
    for t in &catalog {
        for _ in 0..snap.pool_taken.get(&t.name).copied().unwrap_or(0) {
            pool.take_copy(t.card_id);
        }
    }
    (pool, catalog)
}

// ---------------------------------------------------------------------------------------------
// Known bugs
// ---------------------------------------------------------------------------------------------

/// Failures already reported (by a substring of their signature), tolerated unless
/// `SEAGLASS_FUZZ_STRICT=1`, so that `cargo test` stays green until they are fixed.
const KNOWN_BUGS: &[(&str, &str)] = &[
    (
        "re-synced: Relentless Deflector",
        "Relentless Deflector's sync wipes a Taunt given by a spell \
         (fuzz_findings::relentless_deflector_loses_the_taunt_a_spell_gave_it)",
    ),
    (
        "re-synced: Golden Relentless Deflector",
        "Relentless Deflector's sync wipes a Taunt given by a spell \
         (fuzz_findings::relentless_deflector_loses_the_taunt_a_spell_gave_it)",
    ),
    (
        "hand has # cards",
        "Buy runs the gold_spent observers before the card reaches the hand \
         (fuzz_findings::a_lockbox_got_on_a_buy_overfills_the_hand)",
    ),
    (
        "the Discover card did not reach the hand",
        "ChooseDiscover deals its Tier damage before adding the card \
         (fuzz_findings::a_discovered_card_is_lost_to_the_cards_its_damage_gives)",
    ),
    (
        "transient state set: pending_choice_target",
        "a Choose One followed by a Triple Reward leaves pending_choice_target set \
         (fuzz_findings::a_choose_one_before_a_triple_reward_leaves_its_target_set)",
    ),
    (
        "left the pool for nowhere",
        "a card Discovered with a full hand is dropped without returning to the pool \
         (fuzz_findings::a_card_discovered_with_a_full_hand_leaves_the_pool_for_good)",
    ),
    (
        "a minion in the hand changes when the auras are re-synced",
        "Buy runs the gold_spent observers before the card reaches the hand, so it misses \
         their aura changes (fuzz_findings::a_bought_undead_misses_the_undead_bonus_its_gold_gave)",
    ),
    (
        "aren't the units that didn't die",
        "a minion a Deathrattle destroys stays at 0 Health, where a buff revives it \
         (fuzz_findings::a_minion_a_deathrattle_destroys_comes_back_with_a_buff)",
    ),
    (
        "shop has # cards",
        "Demon Fodder Refreshes have no cap on the Tavern's size (rule question)",
    ),
];

fn known_bug(failure: &Failure) -> Option<&'static str> {
    KNOWN_BUGS
        .iter()
        .find(|(pattern, _)| failure.signature.contains(pattern))
        .map(|(_, why)| *why)
}

// ---------------------------------------------------------------------------------------------
// The run
// ---------------------------------------------------------------------------------------------

/// The failures of a run, grouped by signature: the count and the earliest failure of each.
#[derive(Default)]
struct Groups {
    groups: BTreeMap<String, (usize, Failure)>,
    games: u64,
    /// How many times each card was played from hand, over all games.
    played: BTreeMap<u32, u32>,
}

impl Groups {
    fn add(&mut self, failure: Failure) {
        match self.groups.get_mut(&failure.signature) {
            Some((count, first)) => {
                *count += 1;
                let earlier = (failure.call.index, failure.seed) < (first.call.index, first.seed);
                if earlier {
                    *first = failure;
                }
            }
            None => {
                self.groups.insert(failure.signature.clone(), (1, failure));
            }
        }
    }
}

fn run_all(settings: &Settings) -> Groups {
    let groups = Mutex::new(Groups::default());
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let running: Mutex<Vec<Option<(u64, Instant)>>> = Mutex::new(vec![None; settings.threads]);
    let started = Instant::now();
    std::thread::scope(|scope| {
        for worker in 0..settings.threads {
            let (groups, next, done, running) = (&groups, &next, &done, &running);
            scope.spawn(move || loop {
                let i = next.fetch_add(1, Ordering::Relaxed) as u64;
                if i >= settings.games {
                    break;
                }
                let seed = settings.first_seed + i;
                running.lock().unwrap()[worker] = Some((seed, Instant::now()));
                let (failures, played) = fuzz_game(seed, settings);
                running.lock().unwrap()[worker] = None;
                let mut g = groups.lock().unwrap();
                g.games += 1;
                for (card, n) in played {
                    *g.played.entry(card).or_default() += n;
                }
                for f in failures {
                    g.add(f);
                }
                drop(g);
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                if settings.games >= 1000 && n % 1000 == 0 {
                    let g = groups.lock().unwrap();
                    let failures: usize = g.groups.values().map(|(c, _)| c).sum();
                    eprintln!(
                        "[fuzz] {n}/{} games, {failures} failures in {} groups, {:.0?}",
                        settings.games,
                        g.groups.len(),
                        started.elapsed()
                    );
                }
            });
        }
        if settings.threads > 1 {
            // A watchdog for hangs: report games that run for a long time.
            let (done, running) = (&done, &running);
            scope.spawn(move || {
                let mut reported = Vec::new();
                while done.load(Ordering::Relaxed) < settings.games as usize {
                    std::thread::sleep(Duration::from_millis(200));
                    for (seed, since) in running.lock().unwrap().iter().flatten() {
                        if since.elapsed() > Duration::from_secs(60) && !reported.contains(seed) {
                            eprintln!("[fuzz] game {seed} has run for {:.0?}", since.elapsed());
                            reported.push(*seed);
                        }
                    }
                }
            });
        }
    });
    groups.into_inner().unwrap()
}

fn describe_call(call: &Call) -> String {
    let opponent = call
        .opponent
        .map(|o| format!(" v player {o}"))
        .unwrap_or_default();
    let action = call.action.map(|a| format!(" {a:?}")).unwrap_or_default();
    format!(
        "turn {} {:?} player {}{opponent}{action} (call {})",
        call.turn, call.phase, call.player, call.index
    )
}

fn report(groups: &Groups, settings: &Settings) -> String {
    let mut out = String::new();
    let failures: usize = groups.groups.values().map(|(c, _)| c).sum();
    let _ = writeln!(
        out,
        "{} games from seed {} (turns {}, lookahead {}): {failures} failures in {} groups",
        groups.games,
        settings.first_seed,
        settings.turns,
        settings.lookahead,
        groups.groups.len()
    );
    if groups.games >= 1000 {
        // Coverage: the catalog minions and Tavern spells never played from hand.
        let never: Vec<String> = catalog_for("full")
            .expect("full catalog")
            .iter()
            .map(|t| (t.card_id, t.name.clone()))
            .chain(
                cards::spells::all_spells()
                    .iter()
                    .map(|u| (u.card_id, u.name.clone())),
            )
            .filter(|(id, _)| !groups.played.contains_key(id))
            .map(|(_, name)| name)
            .collect();
        let _ = writeln!(
            out,
            "{} cards played; never played: {}",
            groups.played.len(),
            never.join(", ")
        );
    }
    let mut sorted: Vec<&(usize, Failure)> = groups.groups.values().collect();
    sorted.sort_by_key(|(count, f)| (f.kind, std::cmp::Reverse(*count)));
    for (count, f) in sorted {
        let known = known_bug(f)
            .map(|w| format!(" [known: {w}]"))
            .unwrap_or_default();
        let _ = writeln!(out, "\n{count} x {}{known}", f.signature);
        let _ = writeln!(out, "  first: seed {} {}", f.seed, describe_call(&f.call));
        if !f.cards.is_empty() {
            let _ = writeln!(out, "  cards: {}", f.cards);
        }
        let detail: String = f.detail.chars().take(1500).collect();
        let _ = writeln!(out, "  {detail}");
    }
    out
}

/// Write each group's repro snapshot to `dir`.
fn write_snapshots(groups: &Groups, settings: &Settings, dir: &str) {
    std::fs::create_dir_all(dir).expect("create the output directory");
    for (i, (_, f)) in groups.groups.values().enumerate() {
        if let Some(snap) = snapshot(f, settings) {
            let mut h = DefaultHasher::new();
            f.signature.hash(&mut h);
            let path = format!("{dir}/{:02}-{:016x}-seed{}.yaml", i, h.finish(), f.seed);
            std::fs::write(&path, yaml(&snap)).expect("write a snapshot");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Random battles
// ---------------------------------------------------------------------------------------------

/// A random battle's inputs: two boards of catalog minions and tokens (some Golden, buffed, with
/// extra keywords), hands, player auras (Undead bonus, Deity, the Start of Combat effects of
/// spells) and hero tiers.
#[derive(Clone, Debug, Serialize)]
struct BattleCase {
    seed: u64,
    board_a: Vec<Unit>,
    board_b: Vec<Unit>,
    hand_a: Vec<Unit>,
    hand_b: Vec<Unit>,
    auras_a: PlayerAuras,
    auras_b: PlayerAuras,
    tier_a: u32,
    tier_b: u32,
}

fn random_minion(rng: &mut Rng, minions: &[CardTemplate], tokens: &[Unit]) -> Unit {
    let mut unit = if rng.below(6) == 0 {
        tokens[rng.below(tokens.len())].clone()
    } else {
        minions[rng.below(minions.len())].instantiate()
    };
    if rng.below(5) == 0 {
        unit.make_golden();
    }
    if rng.below(3) == 0 {
        unit.add_stats(rng.below(8) as i32, rng.below(8) as i32);
    }
    let keywords = [
        seaglass::Keyword::Taunt,
        seaglass::Keyword::DivineShield,
        seaglass::Keyword::Windfury,
        seaglass::Keyword::Reborn,
        seaglass::Keyword::Venomous,
        seaglass::Keyword::Stealth,
    ];
    for kw in keywords {
        if rng.below(12) == 0 {
            unit.apply_keyword(kw, false);
        }
    }
    unit
}

fn random_auras(rng: &mut Rng, combat_spells: &[u32]) -> PlayerAuras {
    let mut auras = PlayerAuras::default();
    if rng.below(4) == 0 {
        auras.undead_bonus_attack = rng.below(6) as i32;
    }
    if rng.below(4) == 0 {
        auras.blood_gem_bonus_atk = rng.below(3) as i32;
        auras.blood_gem_bonus_hp = rng.below(3) as i32;
    }
    auras.deity.kind = [DeityKind::CThun, DeityKind::YShaarj, DeityKind::None][rng.below(3)];
    auras.deity.attack = 1 + rng.below(10) as i32;
    auras.deity.health = 1 + rng.below(10) as i32;
    if rng.below(3) == 0 && !combat_spells.is_empty() {
        let card = combat_spells[rng.below(combat_spells.len())];
        auras.add_effect(
            card,
            1 + rng.below(3) as u32,
            seaglass::model::EffectDuration::Game,
        );
    }
    auras
}

impl BattleCase {
    fn random(seed: u64) -> Self {
        let mut rng = Rng::new(seed ^ 0xBA77_1E00_0000_0001);
        let minions = cards::all_templates();
        let tokens = cards::tokens::all_token_minions();
        let spells = cards::spells::all_spells();
        let combat_spells: Vec<u32> = spells
            .iter()
            .map(|u| u.card_id)
            .filter(|&id| {
                let h = cards::hooks(id);
                h.player_start_of_combat.is_some()
                    || h.player_combat_space.is_some()
                    || h.player_combat_summon.is_some()
            })
            .collect();
        let board = |rng: &mut Rng| -> Vec<Unit> {
            (0..rng.below(8))
                .map(|_| random_minion(rng, minions, &tokens))
                .collect()
        };
        let board_a = board(&mut rng);
        let board_b = board(&mut rng);
        let hand = |rng: &mut Rng| -> Vec<Unit> {
            (0..rng.below(4))
                .map(|_| {
                    if rng.below(2) == 0 {
                        random_minion(rng, minions, &tokens)
                    } else {
                        spells[rng.below(spells.len())].clone()
                    }
                })
                .collect()
        };
        let hand_a = hand(&mut rng);
        let hand_b = hand(&mut rng);
        Self {
            board_a,
            board_b,
            hand_a,
            hand_b,
            auras_a: random_auras(&mut rng, &combat_spells),
            auras_b: random_auras(&mut rng, &combat_spells),
            tier_a: 1 + rng.below(6) as u32,
            tier_b: 1 + rng.below(6) as u32,
            seed: rng.next_u64(),
        }
    }

    fn state(&self) -> seaglass::GameState {
        seaglass::GameState {
            hero_tier_a: self.tier_a,
            hero_tier_b: self.tier_b,
            auras_a: self.auras_a.clone(),
            auras_b: self.auras_b.clone(),
            hand_a: self.hand_a.clone(),
            hand_b: self.hand_b.clone(),
        }
    }

    fn describe(&self) -> String {
        format!(
            "seed {}: A {} (hand {}, Tier {}) v B {} (hand {}, Tier {})",
            self.seed,
            units_desc(&self.board_a),
            units_desc(&self.hand_a),
            self.tier_a,
            units_desc(&self.board_b),
            units_desc(&self.hand_b),
            self.tier_b
        )
    }
}

/// Play the battle of `case` twice, with the checks of the result (its outcome).
fn fuzz_battle(case: &BattleCase) -> Result<BattleOutcome, Violation> {
    let state = case.state();
    let res = seaglass::resolve_battle(&case.board_a, &case.board_b, &state, case.seed);
    check_battle(&res, case.tier_a, case.tier_b)?;
    check_battle_units(&res, case.board_a.len(), case.board_b.len())?;
    for (side, hand) in [("A", &res.hand_a), ("B", &res.hand_b)] {
        if hand.len() > 10 {
            return Err(broken(
                format!(
                    "side {side}'s hand has {} cards after the battle",
                    hand.len()
                ),
                units_desc(hand),
            ));
        }
    }
    let again = seaglass::resolve_battle(&case.board_a, &case.board_b, &state, case.seed);
    if again != res {
        return Err(violation(
            FailureKind::Nondeterminism,
            "a battle resolves differently twice",
            "",
        ));
    }
    Ok(res.outcome)
}

/// `SEAGLASS_FUZZ_BATTLES` random battles (default 300, from `SEAGLASS_FUZZ_SEED`) keep the
/// combat invariants of the games ([`check_battle`], [`check_battle_units`]).
#[test]
fn random_battles_keep_the_invariants() {
    install_panic_hook();
    let battles: u64 = env_or("SEAGLASS_FUZZ_BATTLES", 300);
    let first_seed: u64 = env_or("SEAGLASS_FUZZ_SEED", 1);
    let threads: usize = env_or("SEAGLASS_FUZZ_THREADS", default_threads()).max(1);
    let strict = std::env::var("SEAGLASS_FUZZ_STRICT").is_ok_and(|v| v == "1");
    let next = AtomicUsize::new(0);
    let groups: Mutex<BTreeMap<String, (usize, String)>> = Mutex::new(BTreeMap::new());
    let outcomes: Mutex<[usize; 3]> = Mutex::new([0; 3]);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed) as u64;
                if i >= battles {
                    break;
                }
                let case = BattleCase::random(first_seed + i);
                CAPTURING.with(|c| *c.borrow_mut() = true);
                let result = panic::catch_unwind(AssertUnwindSafe(|| fuzz_battle(&case)));
                CAPTURING.with(|c| *c.borrow_mut() = false);
                let failure = match result {
                    Ok(Ok(outcome)) => {
                        let i = match outcome {
                            BattleOutcome::AWin => 0,
                            BattleOutcome::BWin => 1,
                            BattleOutcome::Draw => 2,
                        };
                        outcomes.lock().unwrap()[i] += 1;
                        None
                    }
                    Ok(Err(v)) => Some((
                        format!("{:?}: {}", v.kind, normalize(&v.what)),
                        format!("{}: {}", v.what, v.detail),
                    )),
                    Err(_) => {
                        let message = LAST_PANIC
                            .with(|p| p.borrow_mut().take())
                            .unwrap_or_default();
                        let first = message.lines().next().unwrap_or_default().to_string();
                        Some((format!("Crash: {}", normalize(&first)), message))
                    }
                };
                if let Some((signature, detail)) = failure {
                    let mut g = groups.lock().unwrap();
                    let entry = g
                        .entry(signature)
                        .or_insert_with(|| (0, format!("{detail}\n  {}", case.describe())));
                    entry.0 += 1;
                }
            });
        }
    });
    let groups = groups.into_inner().unwrap();
    let [a, b, draws] = outcomes.into_inner().unwrap();
    let mut text = format!(
        "{battles} battles from seed {first_seed} ({a} A wins, {b} B wins, {draws} draws): {} groups\n",
        groups.len()
    );
    for (signature, (count, first)) in &groups {
        let detail: String = first.chars().take(3000).collect();
        let _ = writeln!(text, "\n{count} x {signature}\n  {detail}");
    }
    println!("{text}");
    let unknown: Vec<&String> = groups
        .keys()
        .filter(|sig| strict || !KNOWN_BUGS.iter().any(|(p, _)| sig.contains(p)))
        .collect();
    assert!(
        unknown.is_empty(),
        "new battle failures: {unknown:#?}\n{text}"
    );
}

#[test]
fn random_solo_games_keep_the_invariants() {
    if std::env::var("SEAGLASS_FUZZ_GAME").is_ok() || std::env::var("SEAGLASS_FUZZ_REPRO").is_ok() {
        return;
    }
    let settings = Settings::from_env();
    let groups = run_all(&settings);
    let text = report(&groups, &settings);
    println!("{text}");
    if let Some(dir) = &settings.out {
        write_snapshots(&groups, &settings, dir);
    }
    let unknown: Vec<&String> = groups
        .groups
        .iter()
        .filter(|(_, (_, f))| settings.strict || known_bug(f).is_none())
        .map(|(sig, _)| sig)
        .collect();
    assert!(unknown.is_empty(), "new failures: {unknown:#?}\n{text}");
}

/// `SEAGLASS_FUZZ_GAME=<seed>`: play one game, printing its trace and failure.
#[test]
fn trace_one_game() {
    let Some(seed) = std::env::var("SEAGLASS_FUZZ_GAME")
        .ok()
        .and_then(|v| v.parse().ok())
    else {
        return;
    };
    let settings = Settings::from_env();
    let run = run_game(seed, &settings, true);
    for f in &run.known {
        println!(
            "KNOWN: {}\n  {}\n  {}",
            f.signature,
            describe_call(&f.call),
            f.detail
        );
    }
    match run.failure {
        Some(f) => {
            println!(
                "FAILED: {}\n  {}\n  {}",
                f.signature,
                describe_call(&f.call),
                f.detail
            );
            if let Some(snap) = snapshot(&f, &settings) {
                println!("{}", yaml(&snap));
            }
        }
        None => println!("no failure"),
    }
}

/// A battle's outcome, its checks and its events, one per line.
fn describe_battle(res: &BattleResult) -> String {
    let mut out = format!(
        "{:?}, {} damage; survivors A {} B {}\n",
        res.outcome,
        res.hero_damage,
        units_desc(&res.survivors_a),
        units_desc(&res.survivors_b)
    );
    for (i, e) in res.events.iter().enumerate() {
        let _ = writeln!(out, "  {i:3} {e:?}");
    }
    out
}

/// `SEAGLASS_FUZZ_REPRO=<file>`: replay a snapshot's failing call and print the outcome.
#[test]
fn replay_a_snapshot() {
    let Ok(path) = std::env::var("SEAGLASS_FUZZ_REPRO") else {
        return;
    };
    let text = std::fs::read_to_string(&path).expect("read the snapshot");
    let snap: Snapshot = serde_yaml::from_str(&text).expect("parse the snapshot");
    let (mut pool, _) = snapshot_pool(&snap);
    let mut rng = Rng::new(snap.rng_state);
    let mut state = snap.state.clone();
    println!("{}: {}", snap.signature, snap.detail);
    install_panic_hook();
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| match (snap.phase, snap.action) {
        (Phase::StartTurn, _) => {
            state.start_turn(&mut pool, &mut rng);
            format!("{:?}", check_tavern(&state))
        }
        (Phase::Combat, _) => {
            let mut other = snap.opponent.clone().expect("a combat has an opponent");
            let res = TavernState::resolve_combat_pair(
                &mut state,
                &mut other,
                snap.combat_seed.expect("a combat seed"),
            );
            describe_battle(&res)
        }
        (Phase::Ghost, _) => {
            let other = snap.opponent.clone().expect("a ghost");
            let res = state.resolve_combat_against(
                &other.board,
                other.tavern_tier,
                &other.auras,
                &other.hand,
                snap.combat_seed.expect("a combat seed"),
            );
            describe_battle(&res)
        }
        (_, Some(act)) => {
            let action = TavernAction::from(act);
            let valid = state.valid_actions();
            let legality = check_legality(&state, &valid).map(|unlisted| unlisted.len());
            let result = state.step(action, &mut pool, &mut rng);
            format!(
                "legality {legality:?}; step {result:?}; after: {:?}",
                check_tavern(&state)
            )
        }
        (_, None) => {
            let valid = state.valid_actions();
            format!("{:?}", check_legality(&state, &valid).map(|u| u.len()))
        }
    }));
    match outcome {
        Ok(text) => println!("{text}"),
        Err(_) => println!("panicked"),
    }
    println!(
        "board {}\nhand {}\nshop {}",
        units_desc(&state.board),
        units_desc(&state.hand),
        units_desc(&state.shop)
    );
}

#[test]
fn the_policy_takes_every_kind_of_action() {
    // A check of the fuzzer itself: over a few games, its policy takes the main kinds of action.
    let mut seen = [false; KINDS];
    for seed in 0..4 {
        let mut pool = CardPool::new(catalog_for("full").expect("full catalog"));
        let (mut rng, mut policy) = (Rng::new(seed), Rng::new(!seed));
        let mut state = TavernState::new().with_shop_spells(true);
        for _ in 0..10 {
            state.start_turn(&mut pool, &mut rng);
            for steps in 0..60 {
                let valid = state.valid_actions();
                let a = choose(&state, &valid, &BASE_WEIGHTS, steps, &mut policy);
                seen[kind_of(&state, &a) as usize] = true;
                if state.step(a, &mut pool, &mut rng) == Ok(true) {
                    break;
                }
            }
            state.resolve_combat_against(&[], 1, &PlayerAuras::default(), &[], seed);
        }
    }
    for kind in [
        Kind::BuyMinion,
        Kind::BuySpell,
        Kind::PlayMinion,
        Kind::PlaySpell,
        Kind::Sell,
        Kind::Refresh,
        Kind::Upgrade,
        Kind::EndTurn,
    ] {
        assert!(seen[kind as usize], "the policy never took a {kind:?}");
    }
}
