//! `Trench Fighter` (`BG34_684`) — Tier 3 Quilboar (`3/3`).
//! At the end of your turn, get a (`2` if Golden) `Gem Confiscation`(s).

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 338;
pub const NAME: &str = "Trench Fighter";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 3).with_tribe(Tribe::Quilboar)
}

pub fn on_end_turn(state: &mut TavernState) {
    let mut total = 0u32;
    for u in &state.board {
        if u.card_id == ID {
            total += if u.is_golden { 2 } else { 1 };
        }
    }
    for _ in 0..total {
        state.add_to_hand(tokens::make_gem_confiscation());
    }
}
