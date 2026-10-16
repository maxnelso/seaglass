//! `Corrupted Baton (Greater)` (`BG36_MagicItem_404t`) — Greater, 3 Gold.
//!
//! After you cast a Tavern spell, give your Deity +10/+10.
//!
//! `trinket_spell_cast`: each Tavern spell cast in the Tavern (each cast of a repeated one) gives
//! the player's Deity +10/+10 (`auras.deity`, as `Joyous`).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134323;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_spell_cast(spell_cast))]
}

fn spell_cast(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.auras.deity.attack += 10;
    state.auras.deity.health += 10;
}
