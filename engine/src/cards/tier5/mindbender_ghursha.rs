//! `Mindbender Ghur'sha` (`BG36_097`) — Tier 5 Aberration (`3/9`).
//!
//! Whenever you discard a card, give your other minions `+4/+4` (`+8/+8` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 533;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Mindbender Ghur'sha", 3, 9, 5)
        .with_tribe(Tribe::Aberration)
        .on_after_friendly_discard(after_friendly_discard)
}

pub fn after_friendly_discard(
    state: &mut TavernState,
    self_idx: usize,
    _: &mut CardPool,
    _: &mut Rng,
) {
    let buff = 4 * state.board[self_idx].golden_mult();
    for (idx, u) in state.board.iter_mut().enumerate() {
        if idx != self_idx {
            u.add_stats(buff, buff);
        }
    }
}
