//! `Lichling Hoarder` (`BG36_848`) — Tier 5 Undead (`6/9`).
//!
//! Avenge (3): Get a (`2` if Golden) plain copy(ies) of a minion that started in your warband this combat.
//!
//! Each copy is of a random minion of the starting warband (`docs/combat.md` §2): the board as the
//! combat started, dead or alive, this included; never one summoned during the combat.

use crate::cards::{instantiate_plain_copy, BoardCtx, CardTemplate};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 530;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Lichling Hoarder", 6, 9, 5)
        .with_tribe(Tribe::Undead)
        .on_friendly_death(on_friendly_death)
}

/// Avenge (3), combat deaths only. Copies are picked from the starting warband.
pub fn on_friendly_death(ctx: &mut BoardCtx<'_>, self_idx: usize, _dying: &Unit) {
    if !ctx.in_combat || !ctx.board[self_idx].avenge(3) {
        return;
    }
    let candidates = ctx.start_board;
    if candidates.is_empty() {
        return;
    }
    for _ in 0..ctx.board[self_idx].golden_mult() {
        if ctx.hand.len() >= 10 {
            break;
        }
        let pick = if candidates.len() == 1 {
            0
        } else {
            ctx.rng.below(candidates.len())
        };
        ctx.add_to_hand(instantiate_plain_copy(&candidates[pick]));
    }
}
