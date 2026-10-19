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

use seaglass::{
    with_combat_trace, BattleOutcome, BattleResult, CombatSnapshot, Event, Side, TavernState,
    UnitId,
};
use seaglass_tools::fmt::CombatNames;

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
    /// Another version of the same combat (the engine's, for a `Power.log` combat).
    pub alt: Option<Box<CombatPlayback>>,
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
            format!(
                "{name} · Tier {} · ♥{}{armor}{drop}",
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
