//! Running scenarios (`docs/scenarios.md` §6).
//!
//! Each seed runs the scenario twice: both runs must end in the same state (Tavern state,
//! pool, RNG and every battle), so every scenario also checks determinism. `SCENARIO_TRACE=1`
//! prints every step and the state after it.

use std::fmt::Write as _;
use std::panic::{self, AssertUnwindSafe};

use crate::cards::{self, CardTemplate};
use crate::combat::BattleResult;
use crate::model::{GameState, Unit};
use crate::rng::Rng;
use crate::sim::{self, BattleDistribution};
use crate::tavern::{CardPool, TavernAction, TavernState};

use super::matcher::{self, Captures};
use super::patch;
use super::spec::{
    Action, Body, CombatPlan, FromTavern, Index, Scenario, Setup, Step, StepKind, Zone,
};
use super::units::describe;
use super::view::{show, View};
use super::Failure;

impl Scenario {
    /// Run the scenario: for each seed, twice (the runs must end identically).
    pub fn run(&self) -> Result<(), Failure> {
        let body = self
            .body
            .as_ref()
            .map_err(|e| Failure::Invalid(e.clone()))?;
        let trace = std::env::var("SCENARIO_TRACE").is_ok_and(|v| !v.is_empty() && v != "0");
        for &seed in &body.seeds {
            let prefix = if body.seeds.len() > 1 {
                format!("seed {seed}: ")
            } else {
                String::new()
            };
            let first =
                guarded(|| World::run(body, seed, trace)).map_err(|f| f.prefixed(&prefix))?;
            let second =
                guarded(|| World::run(body, seed, false)).map_err(|f| f.prefixed(&prefix))?;
            if let Some(diff) = first_difference(&first, &second) {
                return Err(Failure::Mismatch(format!(
                    "{prefix}not deterministic: two runs with the same seed ended \
                     differently ({diff})"
                )));
            }
        }
        Ok(())
    }
}

/// Run `f`, turning a panic into a failure.
fn guarded<T>(f: impl FnOnce() -> Result<T, Failure>) -> Result<T, Failure> {
    panic::catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "no message".to_string());
        Err(Failure::Mismatch(format!("panicked: {message}")))
    })
}

/// The first line where two fingerprints differ.
fn first_difference(a: &str, b: &str) -> Option<String> {
    if a == b {
        return None;
    }
    let (mut a_lines, mut b_lines) = (a.lines(), b.lines());
    for line in 1.. {
        match (a_lines.next(), b_lines.next()) {
            (Some(x), Some(y)) if x == y => continue,
            (x, y) => {
                return Some(format!(
                    "line {line}: {:?} vs {:?}",
                    x.unwrap_or("<end>").trim(),
                    y.unwrap_or("<end>").trim()
                ))
            }
        }
    }
    unreachable!()
}

struct Tavern {
    state: TavernState,
    pool: CardPool,
    rng: Rng,
    catalog: Vec<CardTemplate>,
}

struct World {
    seed: u64,
    tavern: Option<Tavern>,
    battles: Vec<BattleResult>,
    distribution: Option<BattleDistribution>,
    captures: Captures,
    trace: bool,
}

fn no_tavern() -> Failure {
    Failure::Invalid(
        "this step needs a Tavern (it is not valid in a `combat:` scenario)".to_string(),
    )
}

fn tavern_mut(tavern: &mut Option<Tavern>) -> Result<&mut Tavern, Failure> {
    tavern.as_mut().ok_or_else(no_tavern)
}

fn zone(state: &TavernState, zone: Zone) -> &Vec<Unit> {
    match zone {
        Zone::Board => &state.board,
        Zone::Hand => &state.hand,
        Zone::Shop => &state.shop,
    }
}

fn zone_mut(state: &mut TavernState, zone: Zone) -> &mut Vec<Unit> {
    match zone {
        Zone::Board => &mut state.board,
        Zone::Hand => &mut state.hand,
        Zone::Shop => &mut state.shop,
    }
}

/// The position `index` refers to among `cards` (a non-negative index is passed on as is, for
/// the engine to check).
fn position(
    cards: &[Unit],
    index: &Index,
    what: &str,
    captures: &Captures,
) -> Result<usize, Failure> {
    match index {
        Index::At(i) if *i >= 0 => Ok(*i as usize),
        Index::At(i) => {
            let len = cards.len() as i64;
            usize::try_from(len + i).map_err(|_| {
                Failure::Mismatch(format!("{what}[{i}]: the {what} has {len} card(s)"))
            })
        }
        Index::Find(matcher) => {
            for (i, card) in cards.iter().enumerate() {
                if matcher::probe(matcher, &View::of(card), captures)? {
                    return Ok(i);
                }
            }
            Err(Failure::Mismatch(format!(
                "no card in the {what} matches {} ({what}: {})",
                show(matcher),
                list(cards)
            )))
        }
    }
}

/// The engine action for `action` in `state`.
fn tavern_action(
    state: &TavernState,
    action: &Action,
    captures: &Captures,
) -> Result<TavernAction, Failure> {
    Ok(match action {
        Action::Buy(i) => TavernAction::Buy {
            shop_index: position(&state.shop, i, "shop", captures)?,
        },
        Action::Sell(i) => TavernAction::Sell {
            board_pos: position(&state.board, i, "board", captures)?,
        },
        Action::Play { hand, pos } => TavernAction::Play {
            hand_index: position(&state.hand, hand, "hand", captures)?,
            board_pos: pos.unwrap_or(state.board.len()),
        },
        Action::Reposition { from, to } => TavernAction::Reposition {
            from_pos: position(&state.board, from, "board", captures)?,
            to_pos: *to,
        },
        Action::Activate { pos, target } => TavernAction::Activate {
            board_pos: position(&state.board, pos, "board", captures)?,
            target_pos: *target,
        },
        Action::Refresh => TavernAction::Refresh,
        Action::UpgradeTavern => TavernAction::UpgradeTavern,
        Action::ToggleFreeze => TavernAction::ToggleFreeze,
        Action::ChooseDiscover(i) => {
            let options = state.discover_pending.as_deref().unwrap_or_default();
            TavernAction::ChooseDiscover {
                option_index: position(options, i, "Discover options", captures)?,
            }
        }
        Action::EndTurn => TavernAction::EndTurn,
    })
}

fn list(cards: &[Unit]) -> String {
    let names: Vec<String> = cards.iter().map(describe).collect();
    format!("[{}]", names.join(", "))
}

impl World {
    /// Run `body` with `seed`; returns a fingerprint of the final state.
    fn run(body: &Body, seed: u64, trace: bool) -> Result<String, Failure> {
        let mut world = World {
            seed,
            tavern: None,
            battles: Vec::new(),
            distribution: None,
            captures: Captures::new(),
            trace,
        };
        match &body.setup {
            Setup::Tavern { catalog, patch } => {
                let catalog = cards::catalog_for(catalog).map_err(Failure::Invalid)?;
                let mut state = TavernState::new();
                patch::apply(&mut state, patch)
                    .map_err(|e| Failure::Invalid(format!("tavern: {e}")))?;
                world.tavern = Some(Tavern {
                    state,
                    pool: CardPool::new(catalog.clone()),
                    rng: Rng::new(seed),
                    catalog,
                });
            }
            Setup::Combat(plan) => {
                let battle = world.simulate(plan)?;
                world.battles.push(battle);
            }
            Setup::Batch(plan, batch) => {
                let (board_a, board_b, state) = world.combatants(plan)?;
                world.distribution = Some(sim::simulate_batch(
                    &board_a,
                    &board_b,
                    &state,
                    batch.base_seed,
                    batch.n,
                ));
            }
        }
        if world.trace {
            println!("--- seed {seed}: setup\n{}", world.summary());
        }
        world.steps(&body.steps, "")?;
        Ok(world.fingerprint())
    }

    fn steps(&mut self, steps: &[Step], prefix: &str) -> Result<(), Failure> {
        for (i, step) in steps.iter().enumerate() {
            let number = format!("{prefix}{}", i + 1);
            match &step.kind {
                StepKind::If {
                    condition,
                    then,
                    otherwise,
                } => {
                    let taken = matcher::probe(condition, &self.view(), &self.captures)
                        .map_err(|f| f.prefixed(&format!("step {number} `if`: ")))?;
                    if self.trace {
                        println!(
                            "--- step {number}: if -> {}",
                            if taken { "then" } else { "else" }
                        );
                    }
                    let (branch, name) = if taken {
                        (then, "then")
                    } else {
                        (otherwise, "else")
                    };
                    self.steps(branch, &format!("{number}.{name}."))?;
                }
                StepKind::Repeat { times, steps } => {
                    for iteration in 1..=*times {
                        self.steps(steps, &format!("{number}[{iteration}/{times}]."))?;
                    }
                }
                kind => {
                    self.leaf(kind)
                        .map_err(|f| f.prefixed(&format!("step {number} `{}`: ", step.desc)))?;
                    if self.trace {
                        println!("--- step {number}: {}\n{}", step.desc, self.summary());
                    }
                }
            }
        }
        Ok(())
    }

    fn leaf(&mut self, kind: &StepKind) -> Result<(), Failure> {
        match kind {
            StepKind::StartTurn => {
                let t = tavern_mut(&mut self.tavern)?;
                t.state.start_turn(&mut t.pool, &mut t.rng);
            }
            StepKind::SyncAllAuras => tavern_mut(&mut self.tavern)?.state.sync_all_auras(),
            StepKind::Action(action) => {
                let t = tavern_mut(&mut self.tavern)?;
                let action = tavern_action(&t.state, action, &self.captures)?;
                t.state.step(action, &mut t.pool, &mut t.rng).map_err(|e| {
                    Failure::Mismatch(format!("the engine rejected {action:?}: {e}"))
                })?;
            }
            StepKind::AddToHand(cards) => {
                let t = tavern_mut(&mut self.tavern)?;
                for card in cards {
                    t.state.add_to_hand(card.clone());
                }
            }
            StepKind::Push(z, cards) => {
                let t = tavern_mut(&mut self.tavern)?;
                zone_mut(&mut t.state, *z).extend(cards.iter().cloned());
            }
            StepKind::Clear(zones) => {
                let t = tavern_mut(&mut self.tavern)?;
                for &z in zones {
                    zone_mut(&mut t.state, z).clear();
                }
            }
            StepKind::Set(fields) => {
                let t = tavern_mut(&mut self.tavern)?;
                patch::apply(&mut t.state, fields).map_err(Failure::Invalid)?;
            }
            StepKind::DealHeroDamage(amount) => tavern_mut(&mut self.tavern)?
                .state
                .deal_hero_damage(*amount),
            StepKind::ApplyGlobalUnitAuras(z, index) => {
                let t = tavern_mut(&mut self.tavern)?;
                let i = position(zone(&t.state, *z), index, z.name(), &self.captures)?;
                let mut unit = zone(&t.state, *z)
                    .get(i)
                    .cloned()
                    .ok_or_else(|| Failure::Mismatch(format!("{}[{i}]: no such card", z.name())))?;
                t.state.apply_global_unit_auras(&mut unit);
                zone_mut(&mut t.state, *z)[i] = unit;
            }
            StepKind::Fight(plan) => {
                let seed = plan.seed.unwrap_or(self.seed);
                let t = tavern_mut(&mut self.tavern)?;
                let battle = t.state.resolve_combat_against(
                    &plan.board,
                    plan.tier,
                    &plan.auras,
                    &plan.hand,
                    seed,
                );
                self.battles.push(battle);
            }
            StepKind::Simulate(plan) => {
                let battle = self.simulate(plan)?;
                self.battles.push(battle);
            }
            StepKind::Expect(expected) => {
                let view = self.view();
                matcher::check(expected, &view, &mut self.captures).map_err(|f| match f {
                    Failure::Mismatch(m) => Failure::Mismatch(format!("{m}\n{}", self.summary())),
                    invalid => invalid,
                })?;
            }
            StepKind::ExpectError(action) => {
                let t = tavern_mut(&mut self.tavern)?;
                let action = tavern_action(&t.state, action, &self.captures)?;
                match t.state.step(action, &mut t.pool, &mut t.rng) {
                    Ok(_) => {
                        return Err(Failure::Mismatch(format!(
                            "expected the engine to reject {action:?}, but it was accepted"
                        )))
                    }
                    Err(e) if e.trim().is_empty() => {
                        return Err(Failure::Mismatch(format!(
                            "the engine rejected {action:?} without a message"
                        )))
                    }
                    Err(_) => {}
                }
            }
            StepKind::ExpectLegal(action, legal) => {
                let t = tavern_mut(&mut self.tavern)?;
                let action = tavern_action(&t.state, action, &self.captures)?;
                if t.state.is_legal(&action) != *legal {
                    return Err(Failure::Mismatch(format!(
                        "expected {action:?} to be {}",
                        if *legal { "legal" } else { "illegal" }
                    )));
                }
            }
            StepKind::If { .. } | StepKind::Repeat { .. } => unreachable!("handled by steps()"),
        }
        Ok(())
    }

    /// Both sides of a standalone combat.
    fn combatants(&self, plan: &CombatPlan) -> Result<(Vec<Unit>, Vec<Unit>, GameState), Failure> {
        let tavern = || self.tavern.as_ref().map(|t| &t.state).ok_or_else(no_tavern);
        let units = |side: &FromTavern<Vec<Unit>>, pick: fn(&TavernState) -> &Vec<Unit>| match side
        {
            FromTavern::Given(units) => Ok(units.clone()),
            FromTavern::Tavern => tavern().map(|state| pick(state).clone()),
        };
        let auras = |side: &FromTavern<_>| match side {
            FromTavern::Given(auras) => Ok(Clone::clone(auras)),
            FromTavern::Tavern => tavern().map(|state| state.auras.clone()),
        };
        let state = GameState {
            hero_tier_a: plan.hero_tier_a,
            hero_tier_b: plan.hero_tier_b,
            auras_a: auras(&plan.auras_a)?,
            auras_b: auras(&plan.auras_b)?,
            hand_a: units(&plan.hand_a, |s| &s.hand)?,
            hand_b: units(&plan.hand_b, |s| &s.hand)?,
        };
        Ok((
            units(&plan.board_a, |s| &s.board)?,
            units(&plan.board_b, |s| &s.board)?,
            state,
        ))
    }

    fn simulate(&self, plan: &CombatPlan) -> Result<BattleResult, Failure> {
        let (board_a, board_b, state) = self.combatants(plan)?;
        Ok(sim::simulate(
            &board_a,
            &board_b,
            &state,
            plan.seed.unwrap_or(self.seed),
        ))
    }

    /// What expectations match against.
    fn view(&self) -> View {
        if let Some(distribution) = &self.distribution {
            return View::of(distribution);
        }
        let combat = self.battles.last().map_or(View::Null, View::of);
        match &self.tavern {
            Some(t) => {
                let mut view = View::of(&t.state);
                let pool = t
                    .catalog
                    .iter()
                    .map(|tpl| {
                        (
                            tpl.name.clone(),
                            View::Int(t.pool.remaining_copies(tpl.card_id).into()),
                        )
                    })
                    .collect();
                view.push("pool", View::CardMap(pool, None));
                view.push("combat", combat);
                view
            }
            None => combat,
        }
    }

    /// A compact summary of the state (for failures and traces).
    fn summary(&self) -> String {
        let mut out = String::new();
        if let Some(d) = &self.distribution {
            let damage = &d.damage;
            let _ = write!(
                out,
                "  batch: {} battles, a_win_rate {}, b_win_rate {}, draw_rate {}, \
                 damage {}..{} (mean {})",
                d.battles,
                d.a_win_rate,
                d.b_win_rate,
                d.draw_rate,
                damage.min,
                damage.max,
                damage.mean
            );
            return out;
        }
        if let Some(t) = &self.tavern {
            let s = &t.state;
            let _ = writeln!(
                out,
                "  tavern: turn {}, tier {}, gold {}/{}, health {}, armor {}{}",
                s.turn,
                s.tavern_tier,
                s.gold,
                s.max_gold,
                s.health,
                s.armor,
                if s.is_frozen { ", frozen" } else { "" }
            );
            let _ = writeln!(out, "  board: {}", list(&s.board));
            let _ = writeln!(out, "  hand: {}", list(&s.hand));
            let _ = write!(out, "  shop: {}", list(&s.shop));
            if let Some(options) = &s.discover_pending {
                let _ = write!(out, "\n  discover: {}", list(options));
            }
        }
        if let Some(b) = self.battles.last() {
            if !out.is_empty() {
                out.push('\n');
            }
            let _ = write!(
                out,
                "  combat: {:?}, hero_damage {}, survivors_a {}, survivors_b {}",
                b.outcome,
                b.hero_damage,
                list(&b.survivors_a),
                list(&b.survivors_b)
            );
        }
        out
    }

    /// Everything a run produced, for the determinism check.
    fn fingerprint(&self) -> String {
        let tavern = self.tavern.as_ref().map(|t| (&t.state, &t.pool, &t.rng));
        format!("{:#?}", (tavern, &self.battles, &self.distribution))
    }
}
