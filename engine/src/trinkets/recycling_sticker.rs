//! `Recycling Sticker` (`BG32_MagicItem_888`) — Greater, 3 Gold, Elemental.
//!
//! After you play an Elemental, gain a free Refresh.
//!
//! `trinket_after_play`: each Elemental played from the hand gives a free Refresh
//! (`free_refreshes`, which carries over like any free Refresh).

use crate::cards::{CardHooks, Played};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 120705;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_play(after_play))]
}

fn after_play(state: &mut TavernState, played: &Played, _: &mut CardPool, _: &mut Rng) {
    if played.from_hand && !played.magnetized && played.tribe.matches(Tribe::Elemental) {
        state.auras.free_refreshes += 1;
    }
}
