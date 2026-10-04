//! `Twilight Tidehunter` (`BG36_703`) — Tier 4 Murloc (`4/6`).
//!
//! Whenever you cast a spell on this, give the left-most minion in your hand `+8/+8` (`+16/+16` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 458;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Twilight Tidehunter", 4, 6, 4)
        .with_tribe(Tribe::Murloc)
        .on_after_targeted_spell(after_targeted_spell)
}

pub fn after_targeted_spell(
    state: &mut TavernState,
    self_idx: usize,
    target_pos: usize,
    _: &mut CardPool,
    _: &mut Rng,
) {
    if self_idx != target_pos {
        return;
    }
    let buff = 8 * state.board[self_idx].golden_mult();
    if let Some(h) = state.hand.iter_mut().find(|u| !u.is_spell) {
        h.add_stats(buff, buff);
    }
}
