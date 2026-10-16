//! `Bloodbound Earrings (Greater)` (`BG32_MagicItem_808t`) — Greater, 6 Gold, Quilboar.
//!
//! After you cast 5 spells, this plays two Blood Gems on all your minions. (5 left!)
//!
//! `trinket_after_spell`: every spell the player casts in the Tavern counts, each of its casts
//! (`counter`, across turns; spells cast in combat don't count: **engine decision**); each
//! 5th plays 2 Blood Gem(s) on every minion on the board (played, not cast: `Unit::play_blood_gems`).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

use super::SpellCast;

pub const ID: CardId = 121038;

/// Spells per reward.
const SPELLS: i32 = 5;

/// Blood Gems played on each minion.
const GEMS: u32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_spell(after_spell))]
}

fn after_spell(state: &mut TavernState, spell: &SpellCast, _: &mut CardPool, _: &mut Rng) {
    for _ in 0..spell.casts {
        if super::count_to(&mut state.auras, ID, SPELLS) {
            super::play_blood_gems_on_board(state, GEMS);
        }
    }
}
