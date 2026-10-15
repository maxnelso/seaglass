//! Running scenarios (`docs/scenarios.md` §6).
//!
//! Each seed runs the scenario twice: both runs must end in the same state (Tavern state or
//! Duos team, pool, RNG and every battle), so every scenario also checks determinism.
//! `SCENARIO_TRACE=1` prints every step and the state after it.

use std::fmt::Write as _;
use std::panic::{self, AssertUnwindSafe};

use serde_yaml::Value;

use crate::cards::{self, ActivateTargetKind, CardTemplate};
use crate::combat::{BattleResult, DuosResult};
use crate::duos_tavern::DuosTavern;
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

/// The Duos team of a `duos_tavern:` scenario.
struct Team {
    state: DuosTavern,
    pool: CardPool,
    rng: Rng,
    catalog: Vec<CardTemplate>,
}

struct World {
    seed: u64,
    tavern: Option<Tavern>,
    team: Option<Team>,
    battles: Vec<BattleResult>,
    /// The Duos combat of a `duos:` scenario.
    duos: Option<DuosResult>,
    /// The fights of a `duos_tavern:` scenario's team.
    team_fights: Vec<DuosResult>,
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

/// Resolve the target of a Hero Power (`HeroPower` or `HeroPower2`) according to its card's
/// `activate_target`.
fn power_target(
    state: &TavernState,
    card_id: Option<crate::model::CardId>,
    target: Option<&Index>,
    captures: &Captures,
) -> Result<Option<usize>, Failure> {
    let Some(i) = target else {
        return Ok(None);
    };
    let kind = card_id.map(|id| cards::hooks(id).activate_target);
    if matches!(kind, Some(ActivateTargetKind::BoardOrShopMinion)) {
        if let Index::Find(matcher) = i {
            for (idx, card) in state.board.iter().enumerate() {
                if matcher::probe(matcher, &View::of(card), captures)? {
                    return Ok(Some(idx));
                }
            }
            for (idx, card) in state.shop.iter().enumerate() {
                if !card.is_spell && matcher::probe(matcher, &View::of(card), captures)? {
                    return Ok(Some(7 + idx));
                }
            }
            return Err(Failure::Mismatch(format!(
                "no minion on the board or in the shop matches {}",
                show(matcher),
            )));
        }
    }
    let (cards, what) = match kind {
        Some(ActivateTargetKind::ShopCard) => (&state.shop, "shop"),
        Some(ActivateTargetKind::HandCard) => (&state.hand, "hand"),
        _ => (&state.board, "board"),
    };
    Ok(Some(position(cards, i, what, captures)?))
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
        Action::PlayOnShop { hand, shop } => TavernAction::PlayOnShop {
            hand_index: position(&state.hand, hand, "hand", captures)?,
            shop_index: position(&state.shop, shop, "shop", captures)?,
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
        Action::Pass(i) => TavernAction::Pass {
            hand_index: position(&state.hand, i, "hand", captures)?,
        },
        Action::HeroPower(target) => TavernAction::HeroPower {
            target: power_target(
                state,
                state.hero().map(|h| h.power.card_id),
                target.as_ref(),
                captures,
            )?,
        },
        Action::HeroPower2(target) => TavernAction::HeroPower2 {
            target: power_target(
                state,
                state
                    .hero()
                    .and_then(|h| h.second_power.as_ref())
                    .map(|p| p.card_id),
                target.as_ref(),
                captures,
            )?,
        },
        Action::ChooseHero(i) => TavernAction::ChooseHero {
            option_index: match i {
                Index::At(n) => {
                    let len = state.hero_options.len() as i64;
                    let n = if *n < 0 { len + n } else { *n };
                    usize::try_from(n).map_err(|_| {
                        Failure::Mismatch(format!("hero_options[{n}]: {len} hero(es) offered"))
                    })?
                }
                Index::Find(Value::String(name)) => {
                    let id = super::spec::hero_id(name).map_err(Failure::Invalid)?;
                    state
                        .hero_options
                        .iter()
                        .position(|&h| h == id)
                        .ok_or_else(|| {
                            Failure::Mismatch(format!("{name} is not among the hero options"))
                        })?
                }
                Index::Find(other) => {
                    return Err(Failure::Invalid(format!(
                        "`choose_hero` takes an index or a hero name, got {}",
                        show(other)
                    )))
                }
            },
        },
    })
}

/// Run a step on one player's own Tavern that isn't an action (`add_to_hand`, `push_*`,
/// `clear`, `set`, `deal_hero_damage`, `apply_global_unit_auras`, `sync_all_auras`,
/// `take_from_pool`, `play_blood_gems`); `false` if `kind` is not one of them.
fn tavern_step(
    state: &mut TavernState,
    pool: &mut CardPool,
    catalog: &[CardTemplate],
    kind: &StepKind,
    captures: &Captures,
) -> Result<bool, Failure> {
    match kind {
        StepKind::SyncAllAuras => state.sync_all_auras(),
        StepKind::AddToHand(cards) => {
            for card in cards {
                state.add_to_hand(card.clone());
            }
        }
        StepKind::Push(z, cards) => zone_mut(state, *z).extend(cards.iter().cloned()),
        StepKind::Clear(zones) => {
            for &z in zones {
                zone_mut(state, z).clear();
            }
        }
        StepKind::Set(fields) => patch::apply(state, fields).map_err(Failure::Invalid)?,
        StepKind::DealHeroDamage(amount) => state.deal_hero_damage(*amount),
        StepKind::ApplyGlobalUnitAuras(z, index) => {
            let i = position(zone(state, *z), index, z.name(), captures)?;
            let mut unit = zone(state, *z)
                .get(i)
                .cloned()
                .ok_or_else(|| Failure::Mismatch(format!("{}[{i}]: no such card", z.name())))?;
            state.apply_global_unit_auras(&mut unit);
            zone_mut(state, *z)[i] = unit;
        }
        StepKind::TakeFromPool(ids) => take_from_pool(catalog, pool, ids)?,
        StepKind::SetHero(id) => state.set_hero(*id).map_err(Failure::Invalid)?,
        StepKind::OfferHeroes(ids) => state.offer_heroes(ids.clone()),
        StepKind::PlayBloodGems(z, index, count) => {
            let i = position(zone(state, *z), index, z.name(), captures)?;
            let auras = state.auras.clone();
            let unit = zone_mut(state, *z)
                .get_mut(i)
                .ok_or_else(|| Failure::Mismatch(format!("{}[{i}]: no such card", z.name())))?;
            unit.play_blood_gems(*count, &auras);
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// `CardPool::take_copy` for each card, which must be in the catalog.
fn take_from_pool(
    catalog: &[CardTemplate],
    pool: &mut CardPool,
    ids: &[crate::model::CardId],
) -> Result<(), Failure> {
    for &id in ids {
        if !catalog.iter().any(|tpl| tpl.card_id == id) {
            return Err(Failure::Invalid(format!(
                "{} is not in this Tavern's pool",
                super::view::card_name(id)
            )));
        }
        pool.take_copy(id);
    }
    Ok(())
}

/// The view of a pool: the remaining copies of each catalog card, by name (`null` for a card
/// that isn't in the catalog).
fn pool_view(catalog: &[CardTemplate], pool: &CardPool) -> View {
    let copies = catalog
        .iter()
        .map(|tpl| {
            (
                tpl.name.clone(),
                View::Int(pool.remaining_copies(tpl.card_id).into()),
            )
        })
        .collect();
    View::CardMap(copies, Some(Box::new(View::Null)))
}

/// The total remaining copies in a pool.
fn pool_size(catalog: &[CardTemplate], pool: &CardPool) -> View {
    let total: u32 = catalog
        .iter()
        .map(|tpl| pool.remaining_copies(tpl.card_id))
        .sum();
    View::Int(total.into())
}

fn list(cards: &[Unit]) -> String {
    let names: Vec<String> = cards.iter().map(describe).collect();
    format!("[{}]", names.join(", "))
}

/// An `expect_error` step: the engine must have rejected `action`, with a message.
fn rejected<T>(action: TavernAction, result: Result<T, String>) -> Result<(), Failure> {
    match result {
        Ok(_) => Err(Failure::Mismatch(format!(
            "expected the engine to reject {action:?}, but it was accepted"
        ))),
        Err(e) if e.trim().is_empty() => Err(Failure::Mismatch(format!(
            "the engine rejected {action:?} without a message"
        ))),
        Err(_) => Ok(()),
    }
}

impl World {
    /// Run `body` with `seed`; returns a fingerprint of the final state.
    fn run(body: &Body, seed: u64, trace: bool) -> Result<String, Failure> {
        let mut world = World {
            seed,
            tavern: None,
            team: None,
            battles: Vec::new(),
            duos: None,
            team_fights: Vec::new(),
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
            Setup::DuosTavern { catalog, patch } => {
                let catalog = cards::catalog_for(catalog).map_err(Failure::Invalid)?;
                let mut state = DuosTavern::new();
                patch::apply(&mut state, patch)
                    .map_err(|e| Failure::Invalid(format!("duos_tavern: {e}")))?;
                state.mirror();
                world.team = Some(Team {
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
            Setup::Duos(plan) => {
                world.duos = Some(sim::simulate_duos(
                    &plan.team_a,
                    &plan.team_b,
                    plan.seed.unwrap_or(seed),
                ));
            }
            Setup::DuosBatch(plan, batch) => {
                world.distribution = Some(sim::simulate_duos_batch(
                    &plan.team_a,
                    &plan.team_b,
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
                    self.leaf(kind, step.player)
                        .map_err(|f| f.prefixed(&format!("step {number} `{}`: ", step.desc)))?;
                    if self.trace {
                        println!("--- step {number}: {}\n{}", step.desc, self.summary());
                    }
                }
            }
        }
        Ok(())
    }

    fn leaf(&mut self, kind: &StepKind, player: Option<usize>) -> Result<(), Failure> {
        if let StepKind::Expect(expected) = kind {
            let view = self.view();
            return matcher::check(expected, &view, &mut self.captures).map_err(|f| match f {
                Failure::Mismatch(m) => Failure::Mismatch(format!("{m}\n{}", self.summary())),
                invalid => invalid,
            });
        }
        if self.team.is_some() {
            return self.team_leaf(kind, player);
        }
        match kind {
            StepKind::StartTurn => {
                let t = tavern_mut(&mut self.tavern)?;
                t.state.start_turn(&mut t.pool, &mut t.rng);
            }
            StepKind::Action(action) => {
                let t = tavern_mut(&mut self.tavern)?;
                let action = tavern_action(&t.state, action, &self.captures)?;
                t.state.step(action, &mut t.pool, &mut t.rng).map_err(|e| {
                    Failure::Mismatch(format!("the engine rejected {action:?}: {e}"))
                })?;
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
            StepKind::ExpectError(action) => {
                let t = tavern_mut(&mut self.tavern)?;
                let action = tavern_action(&t.state, action, &self.captures)?;
                rejected(action, t.state.step(action, &mut t.pool, &mut t.rng))?;
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
            other => {
                let t = tavern_mut(&mut self.tavern)?;
                if !tavern_step(&mut t.state, &mut t.pool, &t.catalog, other, &self.captures)? {
                    return Err(Failure::Invalid(
                        "this step needs a Duos team (a `duos_tavern:` scenario)".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    /// A step of a `duos_tavern:` scenario: the team's, or a player's (`player`).
    fn team_leaf(&mut self, kind: &StepKind, player: Option<usize>) -> Result<(), Failure> {
        let team = self.team.as_mut().expect("a Duos team");
        let for_player =
            || player.ok_or_else(|| Failure::Invalid("this step needs a `player:`".to_string()));
        match kind {
            StepKind::StartTurn => team.state.start_turn(&mut team.pool, &mut team.rng),
            StepKind::DuosFight(plan) => {
                let seed = plan.seed.unwrap_or(self.seed);
                let fight = team.state.resolve_combat_against(&plan.team, seed);
                self.team_fights.push(fight);
            }
            StepKind::TakeFromPool(ids) => take_from_pool(&team.catalog, &mut team.pool, ids)?,
            StepKind::Set(fields) if player.is_none() => {
                patch::apply(&mut team.state, fields).map_err(Failure::Invalid)?;
                team.state.mirror();
            }
            StepKind::Action(action) => {
                let p = for_player()?;
                let action = tavern_action(&team.state.players[p], action, &self.captures)?;
                team.state
                    .step(p, action, &mut team.pool, &mut team.rng)
                    .map_err(|e| {
                        Failure::Mismatch(format!(
                            "the engine rejected {action:?} for player {p}: {e}"
                        ))
                    })?;
            }
            StepKind::ExpectError(action) => {
                let p = for_player()?;
                let action = tavern_action(&team.state.players[p], action, &self.captures)?;
                rejected(
                    action,
                    team.state.step(p, action, &mut team.pool, &mut team.rng),
                )?;
            }
            StepKind::ExpectLegal(action, legal) => {
                let p = for_player()?;
                let action = tavern_action(&team.state.players[p], action, &self.captures)?;
                if team.state.is_legal(p, &action) != *legal {
                    return Err(Failure::Mismatch(format!(
                        "expected {action:?} to be {} for player {p}",
                        if *legal { "legal" } else { "illegal" }
                    )));
                }
            }
            StepKind::If { .. } | StepKind::Repeat { .. } => unreachable!("handled by steps()"),
            other => {
                let p = for_player()?;
                let state = &mut team.state.players[p];
                if !tavern_step(state, &mut team.pool, &team.catalog, other, &self.captures)? {
                    return Err(Failure::Invalid(
                        "this step is not valid in a `duos_tavern:` scenario".to_string(),
                    ));
                }
                // What the step did to the player's Health and Armor is the team's.
                team.state.settle();
            }
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
        if let Some(duos) = &self.duos {
            return View::of(duos);
        }
        if let Some(t) = &self.team {
            let mut view = View::of(&t.state);
            let free_passes = (0..2)
                .map(|p| View::Int(t.state.free_passes(p).into()))
                .collect();
            view.push("free_passes", View::List(free_passes));
            view.push("out", View::Bool(t.state.is_out()));
            view.push("pool", pool_view(&t.catalog, &t.pool));
            view.push("pool_size", pool_size(&t.catalog, &t.pool));
            view.push(
                "combat",
                self.team_fights.last().map_or(View::Null, View::of),
            );
            return view;
        }
        let combat = self.battles.last().map_or(View::Null, View::of);
        match &self.tavern {
            Some(t) => {
                let mut view = View::of(&t.state);
                view.push("pool", pool_view(&t.catalog, &t.pool));
                view.push("pool_size", pool_size(&t.catalog, &t.pool));
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
        if let Some(d) = &self.duos {
            let _ = write!(
                out,
                "  duos: {:?}, hero_damage {}",
                d.outcome, d.hero_damage
            );
            duos_players(&mut out, d);
            return out;
        }
        if let Some(t) = &self.team {
            let team = &t.state;
            let _ = write!(
                out,
                "  team: health {}, armor {}, passes {} this turn / {} this game, first fighter \
                 {}, ended {:?}",
                team.health,
                team.armor,
                team.passes_this_turn,
                team.passes_this_game,
                team.first_fighter,
                team.ended
            );
            for (p, s) in team.players.iter().enumerate() {
                let _ = write!(
                    out,
                    "\n  player {p}: turn {}, tier {}, gold {}/{}, free passes {}{}",
                    s.turn,
                    s.tavern_tier,
                    s.gold,
                    s.max_gold,
                    team.free_passes(p),
                    if s.is_frozen { ", frozen" } else { "" }
                );
                let _ = write!(out, "\n    board: {}", list(&s.board));
                let _ = write!(out, "\n    hand: {}", list(&s.hand));
                let _ = write!(out, "\n    shop: {}", list(&s.shop));
                if !team.incoming[p].is_empty() {
                    let _ = write!(out, "\n    incoming: {}", list(&team.incoming[p]));
                }
                if let Some(options) = &s.discover_pending {
                    let _ = write!(out, "\n    discover: {}", list(options));
                }
            }
            if let Some(d) = self.team_fights.last() {
                let _ = write!(
                    out,
                    "\n  combat: {:?}, hero_damage {}",
                    d.outcome, d.hero_damage
                );
                duos_players(&mut out, d);
            }
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
        let team = self.team.as_ref().map(|t| (&t.state, &t.pool, &t.rng));
        format!(
            "{:#?}",
            (
                tavern,
                team,
                &self.battles,
                &self.duos,
                &self.team_fights,
                &self.distribution
            )
        )
    }
}

/// Each player's part of a Duos combat, for the summary.
fn duos_players(out: &mut String, d: &DuosResult) {
    for (team, players) in [("a", &d.team_a), ("b", &d.team_b)] {
        for (player, p) in players.iter().enumerate() {
            let _ = write!(
                out,
                "\n  team_{team}[{player}]{}: survivors {}, hand {}",
                if p.fought { "" } else { " (didn't fight)" },
                list(&p.survivors),
                list(&p.hand)
            );
        }
    }
}
