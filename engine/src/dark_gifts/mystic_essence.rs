//! `Mystic Essence` (`BG36_MidGameEffect_000t5`) — turns 5–8, weight 100.
//!
//! Deathrattle: Get a random Tavern spell.
//!
//! Deathrattle: a random Tavern spell of the player's Tier or lower, drawn as the shop draws one,
//! to the hand, or waiting for room in a full hand (confirmed); in combat, to the combat hand
//! (`docs/dark_gifts.md` §2.2, §11.1).

use crate::cards::{spells, BoardCtx, CardHooks};
use crate::model::{CardId, DarkGift, Unit};

pub const ID: CardId = 132203;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_gift_deathrattle(deathrattle))]
}

fn deathrattle(_: &Unit, _: &DarkGift, ctx: &mut BoardCtx<'_>) {
    let spell = spells::draw_shop_spell(ctx.hero_tier, ctx.auras.duos_game, ctx.rng);
    ctx.add_to_hand_or_wait(spell);
}
