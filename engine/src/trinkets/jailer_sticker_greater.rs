//! `Jailer Sticker (Greater)` (`BG35_MagicItem_733`) — Greater, 2 Gold, Undead.
//!
//! Spellcraft: Destroy a friendly Undead to get 2 random Undead.
//!
//! As the Lesser one (its spell `BG35_MagicItem_733t`): 2 random Undead.

use crate::cards::spells::{targeted_tribe, SPELLCRAFT};
use crate::cards::CardHooks;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131133;
/// Its Spellcraft spell.
pub const SPELL: CardId = 131134;

/// The Undead it gives.
const UNDEAD: u32 = 2;

/// The trinket's and its spell's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            ID,
            CardHooks::EMPTY
                .on_trinket_bought(give_spell)
                .on_trinket_turn_start(give_spell),
        ),
        (
            SPELL,
            targeted_tribe(Tribe::Undead, cast).with_flags(SPELLCRAFT),
        ),
    ]
}

fn give_spell(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.add_to_hand_or_wait(super::spellcraft_spell(SPELL, "Jailer Sticker"));
}

fn cast(state: &mut TavernState, _: &Unit, pos: usize, pool: &mut CardPool, rng: &mut Rng) {
    super::destroy_for_undead(state, pos, UNDEAD, pool, rng);
}
