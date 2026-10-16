//! `Fridge Magnet` (`BG30_MagicItem_545`) — Greater, 0 Gold, Mech.
//!
//! Avenge (3): Get a random Magnetic minion.
//!
//! `trinket_friendly_death`: every 3rd friendly minion that dies in a combat (the count starts
//! again each turn) gives a random Magnetic minion of the player's Tier or lower from the game's
//! catalog (`trinkets::random_minion`) to the combat hand, if it has room.

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 112397;

/// Friendly deaths per Avenge.
const AVENGE: i32 = 3;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_friendly_death(avenge),
    )]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_combat(state, ID);
}

fn avenge(ctx: &mut BoardCtx<'_>, _: &Unit) {
    if !super::avenge(ctx, ID, AVENGE) {
        return;
    }
    if let Some(card) = super::random_minion(ctx, |t| t.instantiate().magnetic) {
        ctx.add_to_hand(card);
    }
}
