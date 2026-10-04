//! `Trench Fighter` (`BG34_684`) — Tier 3 Quilboar (`3/3`).
//! At the end of your turn, get a (`2` if Golden) `Gem Confiscation`(s).

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::tavern::{CardPool, TavernState};
use crate::rng::Rng;

pub const ID: CardId = 338;
pub const NAME: &str = "Trench Fighter";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 3)
        .with_tribe(Tribe::Quilboar)
        .on_end_of_turn(on_end_turn)
}

pub fn on_end_turn(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let count = if state.board[self_idx].is_golden { 2 } else { 1 };
    for _ in 0..count {
        state.add_to_hand(tokens::make_gem_confiscation());
    }
}
