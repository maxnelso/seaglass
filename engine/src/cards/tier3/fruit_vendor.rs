//! `Fruit Vendor` (`BG36_346`) — Tier 3 Neutral (`3/6`).
//! **Activate (1):** Get `2` (`4` if Golden) `Tavern Dish Bananas`.

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 316;
pub const NAME: &str = "Fruit Vendor";
pub const ACTIVATE_COST: u32 = 1;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 6, 3)
        .with_tribe(Tribe::None)
        .with_activate_cost(ACTIVATE_COST)
}

pub fn on_activate(state: &mut TavernState, source_pos: usize) {
    let count = if state.board[source_pos].is_golden {
        4
    } else {
        2
    };
    for _ in 0..count {
        if state.hand.len() < 10 {
            state.hand.push(spells::make_tavern_dish_banana());
        }
    }
}
