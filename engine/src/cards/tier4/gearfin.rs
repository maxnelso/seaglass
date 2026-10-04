//! `Gearfin` (`BG36_764`) — Tier 4 Mech/Murloc (`6/5`).
//!
//! At the end of your turn, get two (`four` if Golden) 1-Cost Tavern spells.

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 422;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Gearfin", 6, 5, 4)
        .with_tribe(Tribe::MechMurloc)
        .on_end_of_turn(on_end_turn)
}

pub fn on_end_turn(state: &mut TavernState, self_idx: usize, _: &mut CardPool, rng: &mut Rng) {
    let count = if state.board[self_idx].is_golden {
        4
    } else {
        2
    };
    for _ in 0..count {
        if state.hand.len() >= 10 {
            break;
        }
        let spell = spells::draw_random_cost_tavern_spell(1, rng);
        state.add_to_hand(spell);
    }
}
