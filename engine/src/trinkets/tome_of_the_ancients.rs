//! `Tome of the Ancients` (`BG36_MagicItem_418`) — Greater, 2 Gold.
//!
//! After you discard 3 cards, your Tavern spells give an extra +1/+1 this game. (3 left!)
//!
//! `trinket_after_discard`: every 3rd card discarded from the hand (`counter`, across turns)
//! raises the Tavern spells' bonus (`auras.spell_bonus_atk` / `spell_bonus_hp`) by +1/+1 for the
//! rest of the game.

use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134759;

/// Discards per improvement.
const DISCARDS: i32 = 3;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_discard(after_discard))]
}

fn after_discard(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    if super::count_to(&mut state.auras, ID, DISCARDS) {
        state.auras.spell_bonus_atk += 1;
        state.auras.spell_bonus_hp += 1;
    }
}
