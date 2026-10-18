//! `Crystallization` (`BGS_Treasures_110`) — Tier 1 Darkmoon Prize.
//!
//! Your Tavern spells give an extra +1/+1 this game.
//!
//! The Tavern spells' bonus (`auras.spell_bonus_atk` / `spell_bonus_hp`, as `Intrepid
//! Botanist`'s) goes up by +1/+1 for the rest of the game.

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 129551;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    state.auras.spell_bonus_atk += 1;
    state.auras.spell_bonus_hp += 1;
}
