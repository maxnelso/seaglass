//! `Inductive Gyroblade` (`BG36_MagicItem_810`) — Lesser, 0 Gold, Mech.
//!
//! At the end of your turn, get a 4/4 Magnetic Satellite. (Improved by each Tavern spell you've cast this turn!)
//!
//! `trinket_end_turn`: a `Satellite` of `4/4` plus `+4/+4` per Tavern spell cast this turn
//! (confirmed: the base amount per spell), to the hand or waiting for room; repeated by the
//! end-of-turn repeat count. `trinket_spell_cast` counts the casts (`counter`, every cast of a
//! Tavern spell, repeats too), from 0 at each turn start.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133062;

/// The Satellite's base stats, and its improvement per Tavern spell.
const STATS: i32 = 4;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_spell_cast(spell_cast)
            .on_trinket_end_turn(end_turn),
    )]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_turn(state, ID);
}

fn spell_cast(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::count_spell_this_turn(state, ID);
}

fn end_turn(state: &mut TavernState, repeats: u32, _: &mut CardPool, _: &mut Rng) {
    super::gyroblade_satellites(state, ID, STATS, repeats);
}
