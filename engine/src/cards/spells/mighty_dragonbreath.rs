//! `Mighty Dragonbreath` — Tier 4 Tavern spell (`2` Gold).
//!
//! Give your minions +3/+2, again if they are Dragons, and again if they have Divine Shield.

use super::spell;
use crate::cards::CardHooks;
use crate::model::{Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Mighty Dragonbreath`: give your minions +3/+2, again if they are Dragons, and again if they
/// have Divine Shield.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(3, 2);
    for u in &mut state.board {
        u.add_stats(atk, hp);
        if u.tribe.matches(Tribe::Dragon) {
            u.add_stats(atk, hp);
        }
        if u.divine_shield {
            u.add_stats(atk, hp);
        }
    }
}
