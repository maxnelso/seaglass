//! `Cutthroat K'Thir` (`BG36_106`) — Tier 4 Aberration (`4/4`).
//!
//! Whenever you discard a card, give this and your Deity `+4/+4` (`+8/+8` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 413;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Cutthroat K'Thir", 4, 4, 4)
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
    state.board[self_idx].add_stats(buff, buff);
    state.auras.deity.attack += buff;
    state.auras.deity.health += buff;
}
