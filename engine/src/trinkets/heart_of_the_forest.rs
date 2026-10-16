//! `Heart of the Forest` (`BG32_MagicItem_801t`) — Greater, 3 Gold, Pirate, Naga.
//!
//! Your Tavern spells give an extra +1/+1. After you cast {4} Tavern spells from hand, improve this. (6 left!)
//!
//! The text's numbers are broken in the data ("{4}", "6 left"). Picked: the Tavern spells' bonus
//! (`auras.spell_bonus_atk` / `spell_bonus_hp`) goes up by +1/+1. `trinket_after_spell`: every
//! 4th Tavern spell cast from the hand (`counter`, across turns) improves it by +1/+1 more
//! (confirmed).

use crate::cards::{spells, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

use super::SpellCast;

pub const ID: CardId = 120160;

/// Tavern spells from the hand per improvement.
const SPELLS: i32 = 4;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(improve)
            .on_trinket_after_spell(after_spell),
    )]
}

fn improve(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.auras.spell_bonus_atk += 1;
    state.auras.spell_bonus_hp += 1;
}

fn after_spell(state: &mut TavernState, spell: &SpellCast, pool: &mut CardPool, rng: &mut Rng) {
    if spell.from_hand
        && spells::is_tavern_spell(spell.card.card_id)
        && super::count_to(&mut state.auras, ID, SPELLS)
    {
        improve(state, pool, rng);
    }
}
