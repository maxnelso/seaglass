//! `Blood Gem` — token spell (not a Tavern spell).
//!
//! Give a minion `+1/+1`, plus your Blood Gem bonuses.

use super::targeted;
use crate::cards::{board_passive, CardFlags, CardHooks, Passive};
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast).with_flags(CardFlags::NOT_TAVERN_SPELL)
}

/// `Blood Gem`: play a Blood Gem on a minion, plus [`Passive::ExtraHandBloodGemCasts`] more.
pub fn cast(state: &mut TavernState, _: &Unit, board_pos: usize, _: &mut CardPool, rng: &mut Rng) {
    if board_pos < state.board.len() {
        let extra = board_passive(&state.board, Passive::ExtraHandBloodGemCasts);
        state.board[board_pos].play_blood_gems(1 + extra, &state.auras);
        crate::cards::resolve_pending_effects(&mut state.board, &state.auras, rng);
    }
}
