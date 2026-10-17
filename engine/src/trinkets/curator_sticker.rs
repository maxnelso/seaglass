//! `Curator Sticker` (`BG32_MagicItem_807`) — Greater, 4 Gold.
//!
//! Get a Golden Mishmash and a 10/10 Amalgam with Venomous.
//!
//! Picked: a Golden `Mishmash` (a minion in no pool, `docs/trinkets.md` §11.9), then an `Amalgam`
//! token whose stats are 10/10 (its own, as a plain copy of it would have them back: **engine
//! decision**) with its Venomous, each to the hand or waiting for room.

use crate::cards::minions::mishmash;
use crate::cards::{tokens, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 120145;

/// The Amalgam's Attack and Health.
const STATS: i32 = 10;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_golden_minion(state, mishmash::ID);
    let mut amalgam = tokens::make_amalgam();
    amalgam.attack = STATS;
    amalgam.health = STATS;
    amalgam.max_attack = STATS;
    amalgam.max_health = STATS;
    amalgam.base_attack = STATS;
    amalgam.base_health = STATS;
    state.add_to_hand_or_wait(amalgam);
}
