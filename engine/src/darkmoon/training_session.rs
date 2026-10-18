//! `Training Session` (`BGS_Treasures_011`) — Tier 3 Darkmoon Prize.
//!
//! Discover a new Hero Power.
//!
//! A Discover of 3 different Hero Powers, as Sir Finley's (`heroes::offer_power_discover`: the
//! implemented ones the player doesn't have); the chosen one replaces the player's Hero Power
//! (`heroes::offer_replacing_power_discover`, `docs/darkmoon_prizes.md` §4.3). Without a hero,
//! nothing (engine decision).

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::heroes;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 65249;

/// Options in the Discover.
const OPTIONS: usize = 3;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, rng: &mut Rng) {
    if state.hero().is_some() {
        heroes::offer_replacing_power_discover(state, OPTIONS, rng);
    }
}
