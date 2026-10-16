//! `Comfy Coffin (Greater)` (`BG30_MagicItem_547t`) — Greater, 4 Gold, Undead.
//!
//! Whenever you cast a Tavern spell, your Undead have +2 Attack this game (wherever they are).
//!
//! As the Lesser one (`comfy_coffin.rs`), +2 Attack.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 115226;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_spell_cast(spell_cast))]
}

fn spell_cast(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::buff_undead_attack(state, 2);
}
