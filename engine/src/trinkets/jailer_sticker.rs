//! `Jailer Sticker` (`BG35_MagicItem_306`) — Lesser, 2 Gold, Undead.
//!
//! Spellcraft: Destroy a friendly Undead to get a random Undead.
//!
//! Picked, and at each turn start: its Spellcraft spell, `Jailer Sticker` (`BG35_MagicItem_306t`),
//! a temporary token spell (`docs/trinkets.md` §11.6). Cast on a friendly Undead: it is destroyed
//! (`TavernState::destroy_board_unit`), then a random Undead of the player's Tier or lower from
//! the pool goes to the hand (as `The Jailer`'s Rune of Damnation).

use crate::cards::spells::{targeted_tribe, SPELLCRAFT};
use crate::cards::CardHooks;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131135;
/// Its Spellcraft spell.
pub const SPELL: CardId = 131136;

/// The Undead it gives.
const UNDEAD: u32 = 1;

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
