//! `Nomi Sticker` (`BG30_MagicItem_544`) — Lesser, 0 Gold, Elemental.
//!
//! After you play an Elemental, give Elementals in the Tavern +3/+2 this game.
//!
//! `trinket_after_play`: each Elemental played from the hand raises the Tavern's Elemental bonus
//! by +3/+2 for the rest of the game (`tavern_elemental_atk` / `hp`, as `Dune Dweller`'s), and
//! the Elementals in the Tavern now gain it.

use crate::cards::{CardHooks, Played};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 112392;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_play(after_play))]
}

fn after_play(state: &mut TavernState, played: &Played, _: &mut CardPool, _: &mut Rng) {
    if played.from_hand && !played.magnetized && played.tribe.matches(Tribe::Elemental) {
        super::buff_tavern_elementals(state, 3, 2);
    }
}
