//! `Bigwig Bandit` (`BG33_822`) — Tier 4 Pirate (`4/6`).
//!
//! Rally: Get a (`2` if Golden) random Bounty(ies).

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;

pub const ID: CardId = 404;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Bigwig Bandit", 4, 6, 4).with_tribe(Tribe::Pirate)
}

pub fn on_rally(attacker: &Unit, generated_hand: &mut Vec<Unit>, rng: &mut Rng) {
    let count = if attacker.is_golden { 2 } else { 1 };
    for _ in 0..count {
        generated_hand.push(spells::draw_random_bounty(rng));
    }
}
