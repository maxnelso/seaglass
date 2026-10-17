//! `Mystery Cube` (`BG30_MagicItem_703`) — Lesser, 5 Gold.
//!
//! Choose from 2 new Lesser Trinkets to replace this for free. At the start of each turn, repeat this.
//!
//! Picked, and at each turn start while it is kept: an offer (`docs/trinkets.md` §3.5) of 2
//! random Lesser trinkets the player doesn't own (`trinkets::random_trinkets`), free, each
//! replacing the Cube, and the Cube itself, free, which keeps it: the choice can be declined, as
//! the next turn's repeat says **(engine decision**: a third option rather than skipping, which
//! offers don't allow**)**.

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
    let mut options: Vec<TrinketOption> =
        super::random_trinkets(state, false, Tribe::None, NEW, rng)
            .into_iter()
            .map(|id| super::option_replacing(state, id, 0, ID, rng))
            .collect();
    options.push(super::option_replacing(state, ID, 0, ID, rng));
    super::push_offer(state, options);
}
