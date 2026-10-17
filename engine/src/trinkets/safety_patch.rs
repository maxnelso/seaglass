//! `Safety Patch` (`BG35_MagicItem_820`) — Greater, 0 Gold.
//!
//! Cast Ice Block. Gain 5 Gold.
//!
//! Picked: 5 Gold, and *Ice Block* (`armed`): "When your hero takes fatal damage, prevent it and
//! become Immune this turn". `trinket_prevent_fatal`: the first combat damage that would bring the
//! hero (Duos: the team) to 0 Health or less is prevented, all of it, and the Ice Block is used up
//! (`docs/trinkets.md` §6). Being Immune for the rest of the turn changes nothing more: combat
//! damage comes once per turn **(engine decision)**. It is no spell cast.

use crate::cards::CardHooks;
use crate::model::{CardId, TrinketState};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130838;

/// The Gold it gives.
const GOLD: u32 = 5;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_prevent_fatal(prevent_fatal),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.gold += GOLD;
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        t.armed = 1;
    }
}

fn prevent_fatal(trinket: &mut TrinketState) -> bool {
    if trinket.armed == 0 {
        return false;
    }
    trinket.armed = 0;
    true
}
