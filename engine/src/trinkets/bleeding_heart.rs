//! `Bleeding Heart` (`BG30_MagicItem_713`) — Lesser, 2 Gold, Undead.
//!
//! After 8 friendly minions die, get a random Undead. (8 left!)
//!
//! `trinket_friendly_death`: every 8th friendly minion that dies in combat (`counter`, counted
//! across combats, as Ini Stormcoil's) gives a random Undead of the player's Tier or lower from
//! the game's catalog (`trinkets::random_minion`) to the combat hand, if it has room; it comes
//! back with the hand.

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 117906;

/// Friendly deaths per card.
const DEATHS: i32 = 8;

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
    if let Some(card) = super::random_minion(ctx, |t| t.tribe.matches(Tribe::Undead)) {
        ctx.add_to_hand(card);
    }
}
