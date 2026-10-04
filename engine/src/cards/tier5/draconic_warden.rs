//! `Draconic Warden` (`BG34_633`) — Tier 5 Dragon (`7/4`).
//!
//! Battlecry and Deathrattle: Get a (`2` if Golden) random Chromadrake(s).

use crate::cards::{tokens, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 511;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Draconic Warden", 7, 4, 5)
        .with_tribe(Tribe::Dragon)
        .on_battlecry(|state, unit, _, _, rng| on_battlecry(state, unit, rng))
        .on_deathrattle(on_deathrattle)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, rng: &mut Rng) {
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let cid = tokens::CHROMADRAKE_IDS[rng.below(tokens::CHROMADRAKE_IDS.len())];
        state.add_to_hand(tokens::make_chromadrake(cid, false));
    }
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let cid = tokens::CHROMADRAKE_IDS[ctx.rng.below(tokens::CHROMADRAKE_IDS.len())];
        ctx.add_to_hand(tokens::make_chromadrake(cid, false));
    }
}
