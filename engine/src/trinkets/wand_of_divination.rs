//! `Wand of Divination` (`BG36_MagicItem_307`) — Lesser, 0 Gold, Quilboar, Naga.
//!
//! After you cast 3 spells on minions, gain 1 Gold. (3 left!)
//!
//! `trinket_after_spell`: every cast of a spell on a minion (on the board or in the Tavern) counts
//! (`counter`, across turns); each 3rd gives 1 Gold.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

use super::SpellCast;

pub const ID: CardId = 133379;

/// Spells on minions per Gold.
const SPELLS: i32 = 3;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_spell(after_spell))]
}

fn after_spell(state: &mut TavernState, spell: &SpellCast, _: &mut CardPool, _: &mut Rng) {
    if !spell.targeted {
        return;
    }
    for _ in 0..spell.casts {
        if super::count_to(&mut state.auras, ID, SPELLS) {
            state.gold += 1;
        }
    }
}
