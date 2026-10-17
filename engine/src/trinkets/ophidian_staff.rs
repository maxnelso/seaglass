//! `Ophidian Staff` (`BG35_MagicItem_872`) — Lesser, 4 Gold, Beast.
//!
//! Spellcraft: Give a Beast +2/+2 and Reborn.
//!
//! Picked, and at each turn start: its Spellcraft spell, `Ophidian Staff` (`BG35_MagicItem_872t`),
//! a temporary token spell discarded at the end of the turn (`docs/trinkets.md` §11.6), to the hand
//! or waiting for room. Cast on a friendly Beast: +2/+2 (not a Tavern spell: no spell bonus) and
//! Reborn.

use crate::cards::spells::{targeted_tribe, SPELLCRAFT};
use crate::cards::CardHooks;
use crate::model::{CardId, Keyword, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130908;
/// Its Spellcraft spell.
pub const SPELL: CardId = 130909;

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
            targeted_tribe(Tribe::Beast, cast).with_flags(SPELLCRAFT),
        ),
    ]
}

fn give_spell(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.add_to_hand_or_wait(super::spellcraft_spell(SPELL, "Ophidian Staff"));
}

fn cast(state: &mut TavernState, _: &Unit, pos: usize, _: &mut CardPool, _: &mut Rng) {
    if let Some(beast) = state.board.get_mut(pos) {
        beast.add_stats(2, 2);
        beast.apply_keyword(Keyword::Reborn, false);
    }
}
