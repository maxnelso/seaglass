//! `Forest's Bounty` — Tier 5 Tavern spell (`2` Gold).
//!
//! Choose One - give a minion +6/+6 twice; or your minions +2/+2.

use super::targeted_minion;
use crate::cards::tokens::{make_choice_option, CHOICE_FOREST_ALL, CHOICE_FOREST_SINGLE};
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted_minion(cast)
}

/// `Forest's Bounty`: Choose One - give a minion +6/+6 twice; or your minions +2/+2.
pub fn cast(
    state: &mut TavernState,
    _: &Unit,
    target: MinionSlot,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if state.minion(target).is_some() {
        state.pending_choice_target = Some(target);
        let opt0 = make_choice_option(CHOICE_FOREST_SINGLE, "Forest's Bounty (+6/+6 twice)", false);
        let opt1 = make_choice_option(CHOICE_FOREST_ALL, "Forest's Bounty (+2/+2 to all)", false);
        state.resolve_choose_one(opt0, opt1, pool, rng);
    }
}

/// `Forest's Bounty`: give the target +6/+6 twice.
pub fn choose_single(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(6, 6);
    if let Some(target) = state.choice_target_mut() {
        for _ in 0..2 {
            target.add_stats(atk, hp);
        }
    }
}

/// `Forest's Bounty`: give your minions +2/+2.
pub fn choose_all(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(2, 2);
    for b in &mut state.board {
        b.add_stats(atk, hp);
    }
}
