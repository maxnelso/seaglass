//! `Dead Bellringer` (`BG36_511`) — Tier 4 Undead (`3/6`).
//!
//! Activate (1): Give a different friendly Undead Reborn. Then destroy it to gain `+4/+4` (`+8/+8` if Golden).

use crate::cards::{ActivateTargetKind, CardTemplate};
use crate::model::{CardId, Keyword, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 415;
pub const ACTIVATE_COST: u32 = 1;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Dead Bellringer", 3, 6, 4)
        .with_tribe(Tribe::Undead)
        .with_activate_cost(ACTIVATE_COST)
        .with_activate_target(ActivateTargetKind::BoardOtherUndead)
        .on_activate(|state, source_pos, target_pos, pool, rng| {
            on_activate(state, source_pos, target_pos, pool, rng)
        })
}

pub fn on_activate(
    state: &mut TavernState,
    source_pos: usize,
    target_pos: Option<usize>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let Some(t_pos) = target_pos else {
        return;
    };
    if t_pos >= state.board.len()
        || t_pos == source_pos
        || !state.board[t_pos].tribe.matches(Tribe::Undead)
    {
        return;
    }
    let is_golden = state.board[source_pos].is_golden;
    let buff = if is_golden { 8 } else { 4 };
    state.board[source_pos].add_stats(buff, buff);
    state.board[t_pos].apply_keyword(Keyword::Reborn, false);
    state.destroy_board_unit(t_pos, pool, rng);
}
