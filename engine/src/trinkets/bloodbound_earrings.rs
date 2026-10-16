//! `Bloodbound Earrings` (`BG32_MagicItem_808`) — Lesser, 1 Gold, Quilboar.
//!
//! After you cast 4 spells, this plays a Blood Gem on all your minions. (4 left!)
//!
//! `trinket_after_spell`: every spell the player casts in the Tavern counts, each of its casts
//! (`counter`, across turns; spells cast in combat don't count: **engine decision**); each
//! 4th plays 1 Blood Gem(s) on every minion on the board (played, not cast: `Unit::play_blood_gems`).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

use super::SpellCast;

pub const ID: CardId = 120146;

/// Spells per reward.
const SPELLS: i32 = 4;

/// Blood Gems played on each minion.
const GEMS: u32 = 1;

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
