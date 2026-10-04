//! `Alliance Flag` — Tier 1 Tavern spell (`1` Gold).
//!
//! Choose One - give a minion +3/+1; or +1/+3.

use super::targeted;
use crate::cards::tokens::{make_choice_option, CHOICE_ALLIANCE_ATK, CHOICE_ALLIANCE_HP};
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Alliance Flag`: Choose One - give a minion +3/+1; or +1/+3.
pub fn cast(
    state: &mut TavernState,
    _: &Unit,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if board_pos < state.board.len() {
        state.pending_choice_target = Some(board_pos);
        let opt0 = make_choice_option(CHOICE_ALLIANCE_ATK, "Alliance Flag (+3/+1)", false);
        let opt1 = make_choice_option(CHOICE_ALLIANCE_HP, "Alliance Flag (+1/+3)", false);
        state.resolve_choose_one(opt0, opt1, pool, rng);
    }
}

/// `Alliance Flag`: give the target +3/+1.
pub fn choose_atk(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    if let Some(pos) = state.choice_target() {
        let (atk, hp) = state.auras.spell_stat_buff(3, 1);
        state.board[pos].add_stats(atk, hp);
    }
}

/// `Alliance Flag`: give the target +1/+3.
pub fn choose_hp(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    if let Some(pos) = state.choice_target() {
        let (atk, hp) = state.auras.spell_stat_buff(1, 3);
        state.board[pos].add_stats(atk, hp);
    }
}
