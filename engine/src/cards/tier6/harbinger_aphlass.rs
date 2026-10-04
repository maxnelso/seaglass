//! `Harbinger Aph'lass` (`BGFYM_005`) — Tier 6 Aberration (`3/6`).
//!
//! Whenever you discard a card, give your Deity `+2/+1` (`+4/+2` if Golden) and improve this.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 613;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Harbinger Aph'lass", 3, 6, 6)
        .with_tribe(Tribe::Aberration)
        .on_after_friendly_discard(after_friendly_discard)
}

pub fn after_friendly_discard(
    state: &mut TavernState,
    self_idx: usize,
    _: &mut CardPool,
    _: &mut Rng,
) {
    let aphlass = &mut state.board[self_idx];
    let mult = (1 + aphlass.aphlass_stacks as i32) * aphlass.golden_mult();
    aphlass.aphlass_stacks += 1;
    state.auras.deity.attack += 2 * mult;
    state.auras.deity.health += mult;
}
