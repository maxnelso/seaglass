//! `Ysera` (`TB_BaconShop_HERO_53`) — Armor 17 (Duos 15).
//!
//! Hero Power: `Dream Portal` (`TB_BaconShop_HP_062`), 0 Gold: The Tavern offers an extra Dragon whenever it is Refreshed.
//!
//! Passive. After each Refresh (`hero_after_refresh`), a Dragon of the player's Tier or lower from
//! the pool (`CardPool::draw_by_tribe`) joins the shop after its minions (before its spell),
//! beyond its usual size, if the Tavern has room (it holds 7 cards: `docs/tavern.md` §3.2).

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState, MAX_SHOP_SIZE};

pub const ID: HeroId = 60370;
pub const POWER: CardId = 61408;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_after_refresh(dream_portal))]
}

/// After a Refresh: an extra Dragon.
fn dream_portal(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if state.shop.len() >= MAX_SHOP_SIZE {
        return;
    }
    let Some(mut dragon) = pool.draw_by_tribe(Tribe::Dragon, None, state.tavern_tier, rng) else {
        return;
    };
    state.apply_shop_auras(&mut dragon);
    let pos = state
        .shop
        .iter()
        .rposition(|u| !u.is_spell)
        .map_or(0, |i| i + 1);
    state.shop.insert(pos, dragon);
}
