//! `The Eye of Dalaran` (`BG30_MagicItem_981`) — Lesser, 2 Gold.
//!
//! Whenever a friendly minion with no type dies, get a random Tavern spell.
//!
//! `trinket_friendly_death`: each friendly minion without a type that dies in combat gives a
//! random Tavern spell of the player's Tier or lower (drawn as the shop draws one) to the combat
//! hand, if it has room.

use crate::cards::{spells, BoardCtx, CardHooks};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 112850;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_friendly_death(death))]
}

fn death(ctx: &mut BoardCtx<'_>, dying: &Unit) {
    if dying.tribe != Tribe::None {
        return;
    }
    let spell = spells::draw_shop_spell(ctx.hero_tier, ctx.auras.duos_game, ctx.rng);
    ctx.add_to_hand(spell);
}
