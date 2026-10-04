//! `Sky Admiral Rogers` (`BG33_823`) — Tier 6 Pirate (`4/5`).
//!
//! After you spend 9 Gold, get a (`2` if Golden) random Bounty.

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 621;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Sky Admiral Rogers", 4, 5, 6).with_tribe(Tribe::Pirate)
}

pub fn on_gold_spent(state: &mut TavernState, amount: u32, rng: &mut Rng) {
    let mut bounties_to_add = 0u32;
    for u in &mut state.board {
        if u.card_id == ID {
            u.gunpowder_gold_progress += amount;
            while u.gunpowder_gold_progress >= 9 {
                u.gunpowder_gold_progress -= 9;
                bounties_to_add += if u.is_golden { 2 } else { 1 };
            }
        }
    }
    for _ in 0..bounties_to_add {
        state.add_to_hand(spells::draw_random_bounty(rng));
    }
}
