//! `Lovely Locket` (`BG36_MagicItem_211`) — Lesser, 0 Gold.
//!
//! After you cast a spell on a friendly minion, this casts it on another friendly minion.
//!
//! `trinket_after_spell`: once a spell cast on a minion on the board is done (and its target is
//! still there), a fresh copy of it is cast by an effect (`spells::cast_spell_by_effect`: a spell
//! the player casts) on a random other minion on the board (one `rng.below(k)`). That cast doesn't
//! trigger the Locket again (`armed` while it resolves); spells on Tavern minions don't count
//! (confirmed).

use crate::cards::{spells, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

use super::SpellCast;

pub const ID: CardId = 133399;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_spell(after_spell))]
}

fn after_spell(state: &mut TavernState, spell: &SpellCast, pool: &mut CardPool, rng: &mut Rng) {
    let Some(MinionSlot::Board(pos)) = spell.target else {
        return;
    };
    if super::state(&state.auras, ID).is_none_or(|t| t.armed != 0) {
        return;
    }
    let others: Vec<usize> = (0..state.board.len()).filter(|&i| i != pos).collect();
    if others.is_empty() {
        return;
    }
    let other = others[rng.below(others.len())];
    let copy = spells::spell_by_id(spell.card.card_id).unwrap_or_else(|| spell.card.clone());
    set_armed(state, 1);
    spells::cast_spell_by_effect(state, copy, MinionSlot::Board(other), pool, rng);
    set_armed(state, 0);
}

fn set_armed(state: &mut TavernState, armed: u32) {
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        t.armed = armed;
    }
}
