//! `Comfy Coffin` (`BG30_MagicItem_547`) — Lesser, 0 Gold, Undead.
//!
//! Whenever you cast a Tavern spell, your Undead have +1 Attack this game (wherever they are).
//!
//! `trinket_spell_cast`: each Tavern spell cast raises the Undead Attack bonus
//! (`auras.undead_bonus_attack`, as `Nerubian Deathswarmer`'s) by 1, and the auras resync.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 112399;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_spell_cast(spell_cast))]
}

fn spell_cast(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::buff_undead_attack(state, 1);
}
