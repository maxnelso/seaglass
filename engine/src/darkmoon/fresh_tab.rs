//! `Fresh Tab` (`BGS_Treasures_025`) — Tier 4 Darkmoon Prize.
//!
//! Gain 12 Gold.
//!
//! 12 Gold now (it can exceed 10).

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 65274;

/// The Gold it gives.
const GOLD: u32 = 12;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    state.gold += GOLD;
}
