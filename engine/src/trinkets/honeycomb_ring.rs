//! `Honeycomb Ring` (`BG36_MagicItem_371`) — Greater, 5 Gold.
//!
//! Your Tavern spells give an extra +1/+1. After you cast a spell on a minion, improve for this turn only.
//!
//! Picked: the Tavern spells' bonus (`auras.spell_bonus_atk` / `spell_bonus_hp`, as `Intrepid
//! Botanist`'s) goes up by +1/+1 for the rest of the game. `trinket_after_spell`: each cast of a
//! spell on a minion (on the board or in the Tavern) adds +1/+1 more until the end of the turn
//! (`counter` holds this turn's improvements, taken back at the next turn start:
//! confirmed).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

use super::SpellCast;

pub const ID: CardId = 133722;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_after_spell(after_spell)
            .on_trinket_turn_start(new_turn),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.auras.spell_bonus_atk += 1;
    state.auras.spell_bonus_hp += 1;
}

fn after_spell(state: &mut TavernState, spell: &SpellCast, _: &mut CardPool, _: &mut Rng) {
    if !spell.targeted || spell.casts == 0 {
        return;
    }
    let n = spell.casts as i32;
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        t.counter += n;
    }
    state.auras.spell_bonus_atk += n;
    state.auras.spell_bonus_hp += n;
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let Some(t) = super::state_mut(&mut state.auras, ID) else {
        return;
    };
    let n = std::mem::take(&mut t.counter);
    state.auras.spell_bonus_atk -= n;
    state.auras.spell_bonus_hp -= n;
}
