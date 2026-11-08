//! A combat broken into playback steps (`match_replay tui`).
//!
//! A combat is a list of board snapshots and a list of events. Each strike becomes two steps:
//! the **strike** (the boards before it, with the attacker, its target, the damage dealt and the
//! minions about to die marked) and the **settle** (the boards after it).
//!
//! - A seaglass replay's combat is re-run from the players' states before it with the frame's
//!   seed inside [`with_combat_trace`] ([`CombatPlayback::from_replay`]), which snapshots both
//!   boards before and after Start of Combat and after every strike. A Duos combat is re-run the
//!   same way ([`CombatPlayback::from_duos`]): the trace also snapshots both boards each time a
//!   warband steps in for its teammate, so the playback follows the relay leg by leg. A ghost
//!   combat is either, against the boards the ghost's players last fought with
//!   ([`CombatPlayback::from_battle`], [`CombatPlayback::from_duos`]).
//! - A `Power.log` combat comes with the boards the log showed before every attack
//!   (`powerlog`), and can carry the engine's version of it ([`CombatPlayback::alt`]).

use std::ops::Range;

use seaglass::combat::MAX_BOARD_SIZE;
use seaglass::model::capped_damage;
use seaglass::{
    resolve_battle, resolve_duos_battle, with_combat_trace, BattleOutcome, BattleResult,
    CombatSnapshot, DuosResult, DuosTavern, DuosTeam, Event, GameState, PlayerAuras, Rng, Side,
    TavernState, Unit, UnitId,
};
use seaglass_tools::fmt::CombatNames;

/// Number of simulated combats per fight for pre-combat outcome probabilities.
pub const ODDS_RUNS: u32 = if cfg!(debug_assertions) { 100 } else { 2500 };

/// Base seed mixed with a combat's seed or turn to sample independent RNG streams.
pub const ODDS_BASE_SEED: u64 = 0x5EA6_1A55_0DD5_2026;

/// Formatted segments of [`CombatOdds`] from one side's perspective.
pub struct OddsParts {
    pub who: String,
    pub win: String,
    pub tie: String,
    pub loss: String,
    pub damage: String,
    pub note: Option<String>,
    pub has_wins: bool,
    pub has_ties: bool,
    pub has_losses: bool,
}

/// Pre-combat outcome distribution across [`ODDS_RUNS`] simulated runs.
///
/// Stored from Side A's perspective; [`Self::parts`] and [`Self::line`] flip it when Side B is at
/// the bottom of the screen.
#[derive(Clone, Debug)]
pub struct CombatOdds {
    pub side_names: [String; 2],
    pub runs: u32,
    pub a_wins: u32,
    pub b_wins: u32,
    pub draws: u32,
    /// Runs where Side A dealt fatal combat damage to Side B.
    pub a_lethals: u32,
    /// Runs where Side B dealt fatal combat damage to Side A.
    pub b_lethals: u32,
    /// Mean signed capped hero damage from Side A's perspective.
    pub mean_damage_a: f64,
    pub min_damage_a: i32,
    pub max_damage_a: i32,
    /// Optional caveat (for example, `backup unknown` or `approx (vanilla bodies)`).
    pub note: Option<String>,
}

fn signed_damage(v: i32) -> String {
    if v == 0 {
        "0".to_string()
    } else {
        format!("{v:+}")
    }
}

impl CombatOdds {
    /// Sample `runs` combats from `base_seed` and aggregate Win / Tie / Loss / Lethal % and capped
    /// hero damage. `lethal_threshold` is `[hp_needed_to_kill_a, hp_needed_to_kill_b]`.
    pub fn sample(
        side_names: [String; 2],
        runs: u32,
        base_seed: u64,
        damage_cap: Option<u32>,
        lethal_threshold: [Option<u32>; 2],
        note: Option<String>,
        mut run: impl FnMut(u64) -> (BattleOutcome, u32),
    ) -> Self {
        let runs = runs.max(1);
        let mut seeder = Rng::new(base_seed);
        let (mut a_wins, mut b_wins, mut draws) = (0, 0, 0);
        let (mut a_lethals, mut b_lethals) = (0, 0);
        let mut total_a: i64 = 0;
        let mut min_a = i32::MAX;
        let mut max_a = i32::MIN;
        for _ in 0..runs {
            let (outcome, raw_damage) = run(seeder.next_u64());
            let dmg = capped_damage(raw_damage, damage_cap);
            let signed = match outcome {
                BattleOutcome::AWin => {
                    a_wins += 1;
                    if lethal_threshold[1].is_some_and(|th| dmg >= th) {
                        a_lethals += 1;
                    }
                    dmg as i32
                }
                BattleOutcome::BWin => {
                    b_wins += 1;
                    if lethal_threshold[0].is_some_and(|th| dmg >= th) {
                        b_lethals += 1;
                    }
                    -(dmg as i32)
                }
                BattleOutcome::Draw => {
                    draws += 1;
                    0
                }
            };
            total_a += i64::from(signed);
            min_a = min_a.min(signed);
            max_a = max_a.max(signed);
        }
        Self {
            side_names,
            runs,
            a_wins,
            b_wins,
            draws,
            a_lethals,
            b_lethals,
            mean_damage_a: total_a as f64 / f64::from(runs),
            min_damage_a: min_a,
            max_damage_a: max_a,
            note,
        }
    }

    pub fn parts(&self, side: Side) -> OddsParts {
        let (who, wins, losses, win_lethals, loss_lethals, mean, min, max) = match side {
            Side::A => (
                &self.side_names[0],
                self.a_wins,
                self.b_wins,
                self.a_lethals,
                self.b_lethals,
                self.mean_damage_a,
                self.min_damage_a,
                self.max_damage_a,
            ),
            Side::B => (
                &self.side_names[1],
                self.b_wins,
                self.a_wins,
                self.b_lethals,
                self.a_lethals,
                -self.mean_damage_a,
                -self.max_damage_a,
                -self.min_damage_a,
            ),
        };
        let pct = |n: u32| 100.0 * f64::from(n) / f64::from(self.runs.max(1));
        let win_lethal = if win_lethals > 0 {
            format!(" (lethal {:.1}%)", pct(win_lethals))
        } else {
            String::new()
        };
        let loss_lethal = if loss_lethals > 0 {
            format!(" (lethal {:.1}%)", pct(loss_lethals))
        } else {
            String::new()
        };
        let mean = if mean.abs() < 1e-9 { 0.0 } else { mean };
        let damage = if min == max {
            format!("dmg {}", signed_damage(min))
        } else {
            format!(
                "avg {mean:+.1} dmg [{}..{}]",
                signed_damage(min),
                signed_damage(max)
            )
        };
        OddsParts {
            who: who.clone(),
            win: format!("Win {:.1}%{win_lethal}", pct(wins)),
            tie: format!("Tie {:.1}%", pct(self.draws)),
            loss: format!("Loss {:.1}%{loss_lethal}", pct(losses)),
            damage,
            note: self.note.clone(),
            has_wins: wins > 0,
            has_ties: self.draws > 0,
            has_losses: losses > 0,
        }
    }

    pub fn line(&self, side: Side) -> String {
        let p = self.parts(side);
        let note = p.note.map(|n| format!(" · {n}")).unwrap_or_default();
        format!(
            "Odds ({}): {} · {} · {} · {}{note}",
            p.who, p.win, p.tie, p.loss, p.damage
        )
    }
}

fn solo_lethal_threshold(state: &TavernState) -> Option<u32> {
    if state.health <= 0 || seaglass::trinkets::prevent_fatal(&mut state.auras.clone()) {
        None
    } else {
        Some((state.health + state.armor.max(0)) as u32)
    }
}

fn duos_lethal_threshold(tavern: &DuosTavern) -> Option<u32> {
    if tavern.health <= 0
        || tavern
            .players
            .iter()
            .any(|p| seaglass::trinkets::prevent_fatal(&mut p.auras.clone()))
    {
        None
    } else {
        Some((tavern.health + tavern.armor.max(0)) as u32)
    }
}

/// Pre-combat odds for a Solo combat between `pre_a` and `pre_b`.
pub fn solo_odds(
    names: [&str; 2],
    pre_a: &TavernState,
    pre_b: &TavernState,
    seed: u64,
) -> CombatOdds {
    let (mut a, mut b) = (pre_a.clone(), pre_b.clone());
    a.sync_all_auras();
    b.sync_all_auras();
    let mut board_a = a.board.clone();
    board_a.truncate(MAX_BOARD_SIZE);
    let mut board_b = b.board.clone();
    board_b.truncate(MAX_BOARD_SIZE);
    let state = GameState {
        hero_tier_a: a.tavern_tier,
        hero_tier_b: b.tavern_tier,
        auras_a: a.auras.clone(),
        auras_b: b.auras.clone(),
        hand_a: a.hand.clone(),
        hand_b: b.hand.clone(),
    };
    let thresholds = [solo_lethal_threshold(&a), solo_lethal_threshold(&b)];
    CombatOdds::sample(
        names.map(str::to_string),
        ODDS_RUNS,
        seed ^ ODDS_BASE_SEED,
        a.lobby.damage_cap,
        thresholds,
        None,
        |s| {
            let r = resolve_battle(&board_a, &board_b, &state, s);
            (r.outcome, r.hero_damage)
        },
    )
}

/// Pre-combat odds for a Solo combat against a ghost board.
pub fn solo_ghost_odds(
    names: [&str; 2],
    pre_a: &TavernState,
    ghost_board: &[Unit],
    ghost_tier: u32,
    seed: u64,
) -> CombatOdds {
    let mut a = pre_a.clone();
    a.sync_all_auras();
    let mut board_a = a.board.clone();
    board_a.truncate(MAX_BOARD_SIZE);
    let state = GameState {
        hero_tier_a: a.tavern_tier,
        hero_tier_b: ghost_tier,
        auras_a: a.auras.clone(),
        auras_b: PlayerAuras::default(),
        hand_a: a.hand.clone(),
        hand_b: Vec::new(),
    };
    let thresholds = [solo_lethal_threshold(&a), None];
    CombatOdds::sample(
        names.map(str::to_string),
        ODDS_RUNS,
        seed ^ ODDS_BASE_SEED,
        a.lobby.damage_cap,
        thresholds,
        None,
        |s| {
            let r = resolve_battle(&board_a, ghost_board, &state, s);
            (r.outcome, r.hero_damage)
        },
    )
}

/// Pre-combat odds for a Duos combat between two teams.
pub fn duos_odds(
    names: [&str; 2],
    pre_a: &DuosTavern,
    pre_b: &DuosTavern,
    seed: u64,
) -> CombatOdds {
    let (mut a, mut b) = (pre_a.clone(), pre_b.clone());
    a.mirror();
    for s in &mut a.players {
        s.sync_all_auras();
    }
    b.mirror();
    for s in &mut b.players {
        s.sync_all_auras();
    }
    let (team_a, team_b) = (a.team(), b.team());
    let thresholds = [duos_lethal_threshold(&a), duos_lethal_threshold(&b)];
    CombatOdds::sample(
        names.map(str::to_string),
        ODDS_RUNS,
        seed ^ ODDS_BASE_SEED,
        a.players[0].lobby.damage_cap,
        thresholds,
        None,
        |s| {
            let r = resolve_duos_battle(&team_a, &team_b, s);
            (r.outcome, r.hero_damage)
        },
    )
}

/// Pre-combat odds for a Duos combat against a ghost team.
pub fn duos_ghost_odds(
    names: [&str; 2],
    pre_a: &DuosTavern,
    ghost_team: &DuosTeam,
    seed: u64,
) -> CombatOdds {
    let mut a = pre_a.clone();
    a.mirror();
    for s in &mut a.players {
        s.sync_all_auras();
    }
    let team_a = a.team();
    let thresholds = [duos_lethal_threshold(&a), None];
    CombatOdds::sample(
        names.map(str::to_string),
        ODDS_RUNS,
        seed ^ ODDS_BASE_SEED,
        a.players[0].lobby.damage_cap,
        thresholds,
        None,
        |s| {
            let r = resolve_duos_battle(&team_a, ghost_team, s);
            (r.outcome, r.hero_damage)
        },
    )
}

/// What a step shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The warbands as they enter the combat.
    Start,
    /// After Start of Combat effects.
    StartOfCombat,
    /// A Duos warband stepped in for its teammate.
    TagIn,
    /// A strike about to land.
    Strike,
    /// The boards after a strike.
    Settle,
    /// The result.
    End,
}

/// One playback step of a combat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub phase: Phase,
    /// The snapshot whose boards the step shows.
    pub snapshot: usize,
    /// The events the step is about (its caption).
    pub events: Range<usize>,
    /// The strikes landed by the end of the step (to line two versions of a combat up).
    pub strike: usize,
    /// Who fights on side A and side B, in words.
    pub labels: [String; 2],
    /// The attacking unit and its side, for a strike.
    pub attacker: Option<(Side, UnitId)>,
    /// The attacked unit and its side, for a strike.
    pub target: Option<(Side, UnitId)>,
    /// The damage each unit takes in the strike.
    pub damage: Vec<(UnitId, i32)>,
    /// The units the strike kills (on the boards before it, gone after it).
    pub dying: Vec<UnitId>,
    /// The units whose Divine Shield the strike pops.
    pub popped: Vec<UnitId>,
    /// A heading to show instead of the phase's (a Duos step-in: `p3 steps in for team 2`).
    pub heading: Option<String>,
}

impl Step {
    pub fn plain(phase: Phase, snapshot: usize, events: Range<usize>, strike: usize) -> Self {
        Self {
            phase,
            snapshot,
            events,
            strike,
            labels: Default::default(),
            attacker: None,
            target: None,
            damage: Vec::new(),
            dying: Vec::new(),
            popped: Vec::new(),
            heading: None,
        }
    }
}

/// One team of a Duos combat as its playback shows it.
#[derive(Clone, Debug)]
pub struct DuosSide {
    /// The team's name, in the result and the combat's texts (`team 2`, `ghost of team 3`).
    pub team: String,
    /// Each player's name (player 0, then player 1), in the step-in captions.
    pub names: [String; 2],
    /// Each player's board label while their warband fights (`p3 · team 2 · Tier 4 · ♥25`).
    pub labels: [String; 2],
    /// Each player's board label at the end (with the Health the team drops to).
    pub end_labels: [String; 2],
    /// The source's players on the team (to draw the shown one at the bottom).
    pub players: Vec<usize>,
}

/// `Trinkets: A, B` for a player's owned trinkets, if any.
pub fn trinkets_clause(s: &TavernState) -> Option<String> {
    let names: Vec<String> = s
        .auras
        .trinkets
        .iter()
        .filter_map(|t| {
            seaglass::trinkets::data(t.card_id).map(seaglass::trinkets::display_name)
        })
        .collect();
    (!names.is_empty()).then(|| format!("Trinkets: {}", names.join(", ")))
}

/// A combat label from a player's Tavern before and after it: `p1 · Tier 3 · ♥25+2`, with
/// ` → ♥20` once `end` if the combat cost them Health (a Duos player's mirrors their team's),
/// followed by their owned trinkets.
pub fn tavern_label(name: &str, pre: &TavernState, post: &TavernState, end: bool) -> String {
    let armor = if pre.armor > 0 {
        format!("+{}", pre.armor)
    } else {
        String::new()
    };
    let drop = if end && post.health + post.armor < pre.health + pre.armor {
        format!(" → ♥{}", post.health)
    } else {
        String::new()
    };
    let trinkets = trinkets_clause(pre)
        .map(|t| format!(" · {t}"))
        .unwrap_or_default();
    format!(
        "{name} · Tier {} · ♥{}{armor}{drop}{trinkets}",
        pre.tavern_tier, pre.health
    )
}

/// The names of the units of a traced combat whose boards don't come from two Taverns: every
/// unit any snapshot shows (the others take the names their events give them).
fn snapshot_names<'a>(labels: [&'a str; 2], snapshots: &[CombatSnapshot]) -> CombatNames<'a> {
    let units = snapshots.iter().flat_map(|s| {
        let a = s.board_a.iter().map(|u| (u.id, Side::A, u.name.clone()));
        let b = s.board_b.iter().map(|u| (u.id, Side::B, u.name.clone()));
        a.chain(b)
    });
    CombatNames::from_units(labels, units)
}

/// The warning for a re-run whose result is not the replay's.
fn diverged(same: bool) -> Option<String> {
    (!same).then(|| "the re-run differs from the replay".into())
}

/// A combat ready to play back.
#[derive(Clone, Debug)]
pub struct CombatPlayback {
    pub snapshots: Vec<CombatSnapshot>,
    /// Every event of the combat in words.
    pub texts: Vec<String>,
    pub steps: Vec<Step>,
    /// The source's players on each side whose Taverns it can show (a Duos team's two; none for
    /// a ghost). The shown player's side is drawn at the bottom.
    pub players: [Vec<usize>; 2],
    /// The result in words (`P1 wins · 5 damage`).
    pub result: String,
    /// What the version is (`real`, `engine`), when a combat has two.
    pub version: &'static str,
    /// Something to warn about (the re-run differs from the replay, where the engine diverges).
    pub warning: Option<String>,
    /// Pre-combat outcome distribution across simulated runs.
    pub odds: Option<CombatOdds>,
    /// Another version of the same combat (the engine's, for a `Power.log` combat).
    pub alt: Option<Box<CombatPlayback>>,
}

/// One side of a Solo combat as its playback shows it.
#[derive(Clone, Debug)]
pub struct SoloSide {
    /// The side's name, in the result and the combat's texts (`p1`, `ghost of p4`).
    pub name: String,
    /// Its board label during the combat.
    pub label: String,
    /// Its board label at the end (with the Health it drops to).
    pub end_label: String,
    /// The source's player on the side, if it can show their Tavern.
    pub players: Vec<usize>,
}

/// `P1 wins · 5 damage`.
pub fn result_text(outcome: BattleOutcome, damage: u32, names: [&str; 2]) -> String {
    match outcome {
        BattleOutcome::AWin => format!("{} wins · {damage} damage", names[0]),
        BattleOutcome::BWin => format!("{} wins · {damage} damage", names[1]),
        BattleOutcome::Draw => "Draw".to_string(),
    }
}

impl CombatPlayback {
    pub fn with_odds(mut self, odds: CombatOdds) -> Self {
        self.odds = Some(odds);
        self
    }

    /// Re-run a seaglass replay's combat of `a` and `b` from their states before it (`pre`) with
    /// `seed`, and break it into steps. `recorded` is the result the replay re-simulated; `post`
    /// are the states after it (for the Health the loser drops to).
    pub fn from_replay(
        [a, b]: [usize; 2],
        [pre_a, pre_b]: [&TavernState; 2],
        [post_a, post_b]: [&TavernState; 2],
        seed: u64,
        recorded: &BattleResult,
        names: [&str; 2],
    ) -> Self {
        let side = |p: usize, name: &str, pre: &TavernState, post: &TavernState| SoloSide {
            name: name.to_string(),
            label: tavern_label(name, pre, post, false),
            end_label: tavern_label(name, pre, post, true),
            players: vec![p],
        };
        Self::from_battle(
            || TavernState::resolve_combat_pair(&mut pre_a.clone(), &mut pre_b.clone(), seed),
            recorded,
            pre_a.lobby.damage_cap,
            [
                side(a, names[0], pre_a, post_a),
                side(b, names[1], pre_b, post_b),
            ],
        )
        .with_odds(solo_odds(names, pre_a, pre_b, seed))
    }

    /// Re-run a Solo combat inside a [`with_combat_trace`] (`run` resolves it: a replay's combat,
    /// or a player against a ghost's boards) and break it into steps. `recorded` is the result
    /// the replay re-simulated, which the lobby's `damage_cap` (if any) capped.
    pub fn from_battle(
        run: impl FnOnce() -> BattleResult,
        recorded: &BattleResult,
        damage_cap: Option<u32>,
        sides: [SoloSide; 2],
    ) -> Self {
        let (mut result, snapshots) = with_combat_trace(run);
        result.hero_damage = capped_damage(result.hero_damage, damage_cap);
        let names = [sides[0].name.as_str(), sides[1].name.as_str()];
        let mut combat_names = snapshot_names(names, &snapshots);
        let texts = result
            .events
            .iter()
            .map(|e| combat_names.describe(e))
            .collect();
        let mut steps = engine_steps(&snapshots, &result.events);
        for step in &mut steps {
            step.labels = if step.phase == Phase::End {
                sides.clone().map(|s| s.end_label)
            } else {
                sides.clone().map(|s| s.label)
            };
        }
        Self {
            result: result_text(result.outcome, result.hero_damage, names),
            version: "",
            warning: diverged(result == *recorded),
            odds: None,
            alt: None,
            players: sides.map(|s| s.players),
            snapshots,
            texts,
            steps,
        }
    }

    /// Re-run a Duos combat inside a [`with_combat_trace`] (`run` resolves it: two teams' combat,
    /// or a team against a ghost team's boards) and break it into steps, leg by leg: each time a
    /// warband is wiped and its teammate's steps in, a [`Phase::TagIn`] step shows the new
    /// warband (with a caption saying who stepped in), and the boards and labels follow it.
    /// `recorded` is the result the replay re-simulated, which the lobby's `damage_cap` (if any)
    /// capped.
    pub fn from_duos(
        run: impl FnOnce() -> DuosResult,
        recorded: &DuosResult,
        damage_cap: Option<u32>,
        sides: [DuosSide; 2],
    ) -> Self {
        let (mut result, snapshots) = with_combat_trace(run);
        result.hero_damage = capped_damage(result.hero_damage, damage_cap);
        let teams = [sides[0].team.as_str(), sides[1].team.as_str()];
        let index = |side: Side| (side == Side::B) as usize;
        let stepped_in = |team: Side, player: usize| {
            let side = &sides[index(team)];
            format!("{} steps in for {}", side.names[player], side.team)
        };
        let mut combat_names = snapshot_names(teams, &snapshots);
        let texts = result
            .events
            .iter()
            .map(|e| match *e {
                Event::WarbandSteppedIn {
                    team,
                    player,
                    first_attacker,
                } => format!(
                    "{} (first attacker: {})",
                    stepped_in(team, player),
                    teams[index(first_attacker)]
                ),
                _ => combat_names.describe(e),
            })
            .collect();
        let mut steps = engine_steps(&snapshots, &result.events);
        // Who fights for each team: the warband that stepped in last by the end of the step.
        let mut fighting = [0, 0];
        for step in &mut steps {
            let mut entered = Vec::new();
            for e in &result.events[step.events.clone()] {
                if let Event::WarbandSteppedIn { team, player, .. } = *e {
                    fighting[index(team)] = player;
                    entered.push(stepped_in(team, player));
                }
            }
            let end = step.phase == Phase::End;
            step.labels = [0, 1].map(|t| {
                let side = &sides[t];
                let labels = if end { &side.end_labels } else { &side.labels };
                labels[fighting[t]].clone()
            });
            // A warband a Start of Combat wiped is stepped in for before the first strike.
            step.heading = match step.phase {
                _ if entered.is_empty() => None,
                Phase::TagIn => Some(entered.join(" · ")),
                Phase::StartOfCombat => Some(format!("Start of Combat · {}", entered.join(" · "))),
                _ => None,
            };
        }
        Self {
            result: result_text(result.outcome, result.hero_damage, teams),
            version: "",
            warning: diverged(result == *recorded),
            odds: None,
            alt: None,
            players: sides.map(|s| s.players),
            snapshots,
            texts,
            steps,
        }
    }

    /// The step of this playback that matches `step` of another version of the combat: the
    /// same phase after as many strikes, or the nearest one.
    pub fn matching_step(&self, step: &Step) -> usize {
        self.steps
            .iter()
            .position(|s| s.strike == step.strike && s.phase == step.phase)
            .or_else(|| self.steps.iter().position(|s| s.strike >= step.strike))
            .unwrap_or(self.steps.len() - 1)
    }
}

/// The side a unit is on in a snapshot.
pub fn side_of(snapshot: &CombatSnapshot, id: UnitId) -> Option<Side> {
    if snapshot.board_a.iter().any(|u| u.id == id) {
        Some(Side::A)
    } else if snapshot.board_b.iter().any(|u| u.id == id) {
        Some(Side::B)
    } else {
        None
    }
}

pub fn ids(snapshot: &CombatSnapshot) -> impl Iterator<Item = UnitId> + '_ {
    snapshot
        .board_a
        .iter()
        .chain(&snapshot.board_b)
        .map(|u| u.id)
}

/// The playback steps of an engine-traced combat (snapshots from [`with_combat_trace`], events
/// from its `BattleResult` or `DuosResult`). Labels are left empty.
pub fn engine_steps(snapshots: &[CombatSnapshot], events: &[Event]) -> Vec<Step> {
    let mut steps = Vec::new();
    let Some(first) = snapshots.first() else {
        // Nothing traced (no combat ran): just the result.
        steps.push(Step::plain(Phase::End, 0, 0..events.len(), 0));
        return steps;
    };
    steps.push(Step::plain(Phase::Start, 0, 0..first.events, 0));
    // A Duos combat traces each warband's Start of Combat: the snapshots before the first
    // strike are entries and Start of Combat.
    let mut strikes = 0;
    for k in 1..snapshots.len() {
        let (before, after) = (&snapshots[k - 1], &snapshots[k]);
        let range = before.events..after.events;
        let events_in = &events[range.clone()];
        let attack = events_in.iter().find_map(|e| match *e {
            Event::AttackDeclared {
                side,
                attacker,
                target,
            } => Some((side, attacker, target)),
            _ => None,
        });
        let Some((side, attacker, target)) = attack else {
            let stepped_in = events_in
                .iter()
                .any(|e| matches!(e, Event::WarbandSteppedIn { .. }));
            let phase = match (stepped_in, strikes > 0) {
                (true, true) => Phase::TagIn,
                (false, true) => Phase::Settle,
                (_, false) => Phase::StartOfCombat,
            };
            if !range.is_empty() {
                steps.push(Step::plain(phase, k, range, strikes));
            }
            continue;
        };
        let mut strike = Step::plain(Phase::Strike, k - 1, range.clone(), strikes);
        strike.attacker = Some((side, attacker));
        strike.target = side_of(before, target).map(|s| (s, target));
        for event in events_in {
            match *event {
                Event::DamageDealt { unit, amount, .. } => {
                    match strike.damage.iter_mut().find(|(id, _)| *id == unit) {
                        Some((_, total)) => *total += amount,
                        None => strike.damage.push((unit, amount)),
                    }
                }
                Event::DivineShieldPopped { unit } => strike.popped.push(unit),
                _ => {}
            }
        }
        let alive_after: Vec<UnitId> = ids(after).collect();
        strike.dying = ids(before).filter(|id| !alive_after.contains(id)).collect();
        strikes += 1;
        steps.push(strike);
        steps.push(Step::plain(Phase::Settle, k, range, strikes));
    }
    let last = snapshots.len() - 1;
    steps.push(Step::plain(
        Phase::End,
        last,
        snapshots[last].events..events.len(),
        strikes,
    ));
    steps
}
