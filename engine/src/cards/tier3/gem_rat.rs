//! `Gem Rat` (`BG31_326`) — Tier 3 Quilboar (`4/4`).
//! At the end of your turn, get a (`2` if Golden) `Gem Day`(s).

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 317;
pub const NAME: &str = "Gem Rat";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 4, 3).with_tribe(Tribe::Quilboar)
}

pub fn on_end_turn(state: &mut TavernState) {
    let mut total = 0u32;
    for u in &state.board {
        if u.card_id == ID {
            total += if u.is_golden { 2 } else { 1 };
        }
    }
    for _ in 0..total {
        if state.hand.len() < 10 {
            state.hand.push(tokens::make_gem_day());
        }
    }
}
