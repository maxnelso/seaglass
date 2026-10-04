//! `Clever Castaway` (`BG36_342`) — Tier 2 Pirate (`2/3`).
//! **Activate (2):** **Discover** a (`2` if Golden) Tavern spell(s).

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 205;
pub const NAME: &str = "Clever Castaway";
pub const ACTIVATE_COST: u32 = 2;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 3, 2)
        .with_tribe(Tribe::Pirate)
        .with_activate_cost(ACTIVATE_COST)
        .on_activate(|state, source_pos, _, _, rng| on_activate(state, source_pos, rng))
}

pub fn on_activate(state: &mut TavernState, source_pos: usize, rng: &mut Rng) {
    let count = if state.board[source_pos].is_golden {
        2
    } else {
        1
    };
    for _ in 0..count {
        let opts = spells::draw_discover_tavern_spells(state.tavern_tier, 3, rng);
        if !opts.is_empty() {
            state.push_discover(opts);
        }
    }
}
