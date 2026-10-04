//! `Gem Rat` (`BG31_326`) — Tier 3 Quilboar (`4/4`).
//! At the end of your turn, get a (`2` if Golden) `Gem Day`(s).

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 317;
pub const NAME: &str = "Gem Rat";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 4, 3)
        .with_tribe(Tribe::Quilboar)
        .on_end_of_turn(on_end_turn)
}

pub fn on_end_turn(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let count = if state.board[self_idx].is_golden {
        2
    } else {
        1
    };
    for _ in 0..count {
        state.add_to_hand(tokens::make_gem_day());
    }
}
