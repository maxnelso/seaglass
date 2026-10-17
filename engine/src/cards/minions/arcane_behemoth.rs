//! `Arcane Behemoth` (`BG31_360`) — Tier 7 Elemental (`4/8`), in no pool.
//! **Taunt**. After you sell an Elemental, gain its stats (`double` its stats if Golden).
//!
//! Only `Behemoth Portrait` gives it (`docs/trinkets.md` §11.9). An `after_friendly_sell`
//! observer: the sold minion is the sale just recorded (`TavernState::sold_this_step`); its
//! Attack and Health as it left the board, permanently.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10803;
pub const NAME: &str = "Arcane Behemoth";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 8, 7)
        .with_tribe(Tribe::Elemental)
        .with_keyword(Keyword::Taunt)
        .on_after_friendly_sell(after_friendly_sell)
}

pub fn after_friendly_sell(
    state: &mut TavernState,
    self_idx: usize,
    _: &mut CardPool,
    _: &mut Rng,
) {
    let Some(sold) = state.sold_this_step.last() else {
        return;
    };
    if !sold.tribe.matches(Tribe::Elemental) {
        return;
    }
    let (attack, health) = (sold.attack, sold.health);
    let behemoth = &mut state.board[self_idx];
    let mult = behemoth.golden_mult();
    behemoth.add_stats(attack * mult, health * mult);
}
