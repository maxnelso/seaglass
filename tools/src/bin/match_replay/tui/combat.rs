//! A combat broken into playback steps (`match_replay tui`).
//!
//! A combat is a list of board snapshots and a list of events. Each strike becomes two steps:
//! the **strike** (the boards before it, with the attacker, its target, the damage dealt and the
//! minions about to die marked) and the **settle** (the boards after it).
//!
//! - A seaglass replay's combat is re-run from the players' states before it with the frame's
//!   seed inside [`with_combat_trace`] ([`CombatPlayback::from_replay`]), which snapshots both
//!   boards before and after Start of Combat and after every strike.
//! - A `Power.log` combat comes with the boards the log showed before every attack
//!   (`powerlog`), and can carry the engine's version of it ([`CombatPlayback::alt`]).

use std::ops::Range;

use seaglass::combat::MAX_BOARD_SIZE;
use seaglass::{
    resolve_battle, with_combat_trace, BattleOutcome, BattleResult, CombatSnapshot, Event,
    GameState, Rng, Side, TavernState, UnitId,
};
use seaglass_tools::fmt::CombatNames;

/// Number of simulated combats per fight for pre-combat outcome probabilities.
pub const ODDS_RUNS: u32 = if cfg!(debug_assertions) { 100 } else { 1000 };

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
            let dmg = damage_cap.map_or(raw_damage, |cap| raw_damage.min(cap));
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
        None,
        thresholds,
        None,
        |s| {
            let r = resolve_battle(&board_a, &board_b, &state, s);
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
        }
    }
}

/// A combat ready to play back.
#[derive(Clone, Debug)]
pub struct CombatPlayback {
    pub snapshots: Vec<CombatSnapshot>,
    /// Every event of the combat in words.
    pub texts: Vec<String>,
    pub steps: Vec<Step>,
    /// The source's player on each side, if it can show their Tavern (the shown player is drawn
    /// at the bottom).
    pub players: [Option<usize>; 2],
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

/// `P1 wins · 5 damage`.
pub fn result_text(outcome: BattleOutcome, damage: u32, names: [&str; 2]) -> String {
    match outcome {
        BattleOutcome::AWin => format!("{} wins · {damage} damage", names[0]),
        BattleOutcome::BWin => format!("{} wins · {damage} damage", names[1]),
        BattleOutcome::Draw => "Draw".to_string(),
    }
}

impl CombatPlayback {
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
        let (result, snapshots) = with_combat_trace(|| {
            TavernState::resolve_combat_pair(&mut pre_a.clone(), &mut pre_b.clone(), seed)
        });
        let mut combat_names = CombatNames::new(names, pre_a, pre_b);
        let texts = result
            .events
            .iter()
            .map(|e| combat_names.describe(e))
            .collect();
        let label = |name: &str, pre: &TavernState, post: &TavernState, end: bool| {
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
        };
        let mut steps = engine_steps(&snapshots, &result.events);
        for step in &mut steps {
            let end = step.phase == Phase::End;
            step.labels = [
                label(names[0], pre_a, post_a, end),
                label(names[1], pre_b, post_b, end),
            ];
        }
        Self {
            players: [Some(a), Some(b)],
            result: result_text(result.outcome, result.hero_damage, names),
            version: "",
            warning: (result != *recorded).then(|| "the re-run differs from the replay".into()),
            odds: Some(solo_odds(names, pre_a, pre_b, seed)),
            alt: None,
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
