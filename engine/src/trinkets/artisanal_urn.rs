//! `Artisanal Urn` (`BG30_MagicItem_989t`) — Greater, 0 Gold, Undead.
//!
//! Your Undead have +20 Attack.
//!
//! Picked: the Undead Attack bonus (`auras.undead_bonus_attack`, as `Nerubian Deathswarmer`'s)
//! goes up by 20 for the rest of the game (the trinket is kept), and the auras resync.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 117157;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::buff_undead_attack(state, 20);
}
