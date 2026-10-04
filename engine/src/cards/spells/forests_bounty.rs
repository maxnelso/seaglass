//! `Forest's Bounty` — Tier 5 Tavern spell (`2` Gold).
//!
//! Choose One - give a minion +6/+6 twice; or your minions +2/+2.

use super::targeted;
use crate::cards::tokens::{make_choice_option, CHOICE_FOREST_ALL, CHOICE_FOREST_SINGLE};
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Forest's Bounty`: Choose One - give a minion +6/+6 twice; or your minions +2/+2.
pub fn cast(
    state: &mut TavernState,
    _: &Unit,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if board_pos < state.board.len() {
        state.pending_choice_target = Some(board_pos);
        let opt0 = make_choice_option(CHOICE_FOREST_SINGLE, "Forest's Bounty (+6/+6 twice)", false);
        let opt1 = make_choice_option(CHOICE_FOREST_ALL, "Forest's Bounty (+2/+2 to all)", false);
        state.resolve_choose_one(opt0, opt1, pool, rng);
    }
}

/// `Forest's Bounty`: give the target +6/+6 twice.
pub fn choose_single(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    if let Some(pos) = state.choice_target() {
        let (atk, hp) = state.auras.spell_stat_buff(6, 6);
        for _ in 0..2 {
            state.board[pos].add_stats(atk, hp);
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
