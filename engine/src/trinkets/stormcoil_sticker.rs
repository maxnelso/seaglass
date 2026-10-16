//! `Stormcoil Sticker` (`BG35_MagicItem_302`) — Lesser, 0 Gold, Mech.
//!
//! After 6 friendly minions die, get a random Mech. (6 left!)
//!
//! `trinket_friendly_death`: every 6th friendly minion that dies in combat (`counter`, counted
//! across combats, as Ini Stormcoil's) gives a random Mech of the player's Tier or lower from
//! the game's catalog (`trinkets::random_minion`) to the combat hand, if it has room; it comes
//! back with the hand.

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 130880;

/// Friendly deaths per card.
const DEATHS: i32 = 6;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_friendly_death(friendly_death),
    )]
}

fn friendly_death(ctx: &mut BoardCtx<'_>, _: &Unit) {
    if !super::avenge(ctx, ID, DEATHS) {
        return;
    }
    if let Some(card) = super::random_minion(ctx, |t| t.tribe.matches(Tribe::Mech)) {
        ctx.add_to_hand(card);
    }
}
