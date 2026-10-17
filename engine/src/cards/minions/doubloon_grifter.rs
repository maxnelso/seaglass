//! `Doubloon Grifter` (`BG31_826`) — Tier 4 Pirate (`6/6`), in no pool.
//! Whenever you get 4 Pirates, increase your maximum Gold by 1 (`2` if Golden). *(4 left!)*
//!
//! Only `Grifter Portrait` gives it (`docs/trinkets.md` §11.9). A `card_gained` observer: each
//! Pirate minion that enters the hand while it is on the board counts (bought, Discovered, given,
//! a triple's Golden), in its own counter (`counter`); every 4th raises the maximum Gold at once
//! and for the rest of the game, as `Strike Oil` **(engine decision)**.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 10801;
pub const NAME: &str = "Doubloon Grifter";
/// Pirates to get per maximum Gold.
const PIRATES: i32 = 4;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 6, 6, 4)
        .with_tribe(Tribe::Pirate)
        .on_card_gained(card_gained)
}

/// A card entered the hand: count it if it is a Pirate.
pub fn card_gained(state: &mut TavernState, self_idx: usize, card: &Unit) {
    if card.is_spell || !card.tribe.matches(Tribe::Pirate) {
        return;
    }
    let grifter = &mut state.board[self_idx];
    grifter.counter += 1;
    if grifter.counter >= PIRATES {
        grifter.counter = 0;
        let gold = grifter.golden_mult() as u32;
        state.auras.base_max_gold_bonus += gold;
        state.max_gold += gold;
    }
}
