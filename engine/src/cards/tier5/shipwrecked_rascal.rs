//! `Shipwrecked Rascal` (`BG33_821`) — Tier 5 Pirate (`5/4`).
//!
//! Battlecry and Deathrattle: Get a (`2` if Golden) random Bounty(ies).

use crate::cards::{spells, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 548;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Shipwrecked Rascal", 5, 4, 5).with_tribe(Tribe::Pirate)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, rng: &mut Rng) {
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        state.add_to_hand(spells::draw_random_bounty(rng));
    }
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let bounty = spells::draw_random_bounty(ctx.rng);
        ctx.add_to_hand(bounty);
    }
}
