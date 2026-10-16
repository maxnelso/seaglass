//! `Corrupted Baton` (`BG36_MagicItem_404`) — Lesser, 2 Gold.
//!
//! After you cast a Tavern spell, give your Deity +4/+4.
//!
//! `trinket_spell_cast`: each Tavern spell cast in the Tavern (each cast of a repeated one) gives
//! the player's Deity +4/+4 (`auras.deity`, as `Joyous`).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134317;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_spell_cast(spell_cast))]
}

fn spell_cast(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.auras.deity.attack += 4;
    state.auras.deity.health += 4;
}
