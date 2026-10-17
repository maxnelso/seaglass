//! `Mystery Cube` (`BG30_MagicItem_703`) — Lesser, 5 Gold.
//!
//! Choose from 2 new Lesser Trinkets to replace this for free. At the start of each turn, repeat this.
//!
//! Picked: an offer (`docs/trinkets.md` §3.5) of 2 random Lesser trinkets the player doesn't own
//! (`trinkets::random_trinkets`), free, each replacing the Cube. The Cube can't be kept: one of
//! the 2 must be picked **(confirmed)**, as any offer must (§3.3). So it repeats at a turn start
//! only if it is still there: when no Lesser trinket was left to offer.

use crate::cards::CardHooks;
use crate::model::{CardId, Tribe, TrinketOption};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 117801;

/// The new trinkets offered.
const NEW: usize = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(offer)
            .on_trinket_turn_start(offer),
    )]
}

fn offer(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    let options: Vec<TrinketOption> = super::random_trinkets(state, false, Tribe::None, NEW, rng)
        .into_iter()
        .map(|id| super::option_replacing(state, id, 0, ID, rng))
        .collect();
    super::push_offer(state, options);
}
