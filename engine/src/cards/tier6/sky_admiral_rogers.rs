//! `Sky Admiral Rogers` (`BG33_823`) — Tier 6 Pirate (`4/5`).
//!
//! After you spend 9 Gold, get a (`2` if Golden) random Bounty.

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 621;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Sky Admiral Rogers", 4, 5, 6)
        .with_tribe(Tribe::Pirate)
        .on_gold_spent(on_gold_spent)
}

pub fn on_gold_spent(
    state: &mut TavernState,
    self_idx: usize,
    amount: u32,
    _: &mut CardPool,
    rng: &mut Rng,
) {
    let rogers = &mut state.board[self_idx];
    rogers.counter += amount as i32;
    let mut bounties = 0;
    while rogers.counter >= 9 {
        rogers.counter -= 9;
        bounties += rogers.golden_mult();
    }
    for _ in 0..bounties {
        state.add_to_hand(spells::draw_random_bounty(rng));
    }
}
