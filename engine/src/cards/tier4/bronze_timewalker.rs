//! `Bronze Timewalker` (`BG36_242`) — Tier 4 Dragon (`4/5`).
//!
//! Rally: Get a (`2` if Golden) random Chromadrake(s).

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;

pub const ID: CardId = 410;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Bronze Timewalker", 4, 5, 4).with_tribe(Tribe::Dragon)
}

pub fn on_rally(attacker: &Unit, generated_hand: &mut Vec<Unit>, rng: &mut Rng) {
    let count = if attacker.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let cid = tokens::CHROMADRAKE_IDS[rng.below(tokens::CHROMADRAKE_IDS.len())];
        generated_hand.push(tokens::make_chromadrake(cid, false));
    }
}
