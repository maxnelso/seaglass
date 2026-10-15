//! `Exarch Othaar` (`BG31_HERO_006`) — Armor 15 (Duos 12).
//!
//! Hero Power: `Arcane Knowledge` (`BG31_HERO_006p`), 0 Gold: The next Tavern spell you buy costs (1) less. (Unlocks on Turn 3.)
//!
//! Active (engine decision: an action), untargeted, once per turn, from the 3rd turn on
//! (`turn >= 3`): `next_spell_discount += 1`.

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 117425;
pub const POWER: CardId = 117426;

/// The turn it unlocks on.
const UNLOCK_TURN: u32 = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(arcane_knowledge)
            .on_hero_power_usable(|state| state.turn >= UNLOCK_TURN),
    )]
}

/// Using it: the next Tavern spell costs 1 less.
fn arcane_knowledge(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    state.auras.next_spell_discount += 1;
}
