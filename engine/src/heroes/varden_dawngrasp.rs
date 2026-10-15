//! `Varden Dawngrasp` (`BG22_HERO_004`) — Armor 18 (Duos 16).
//!
//! Hero Power: `Twice as Nice` (`BG22_HERO_004p`), 0 Gold: After the Tavern is Refreshed, copy its highest-Tier minion and Freeze them both.
//!
//! Passive. After each Refresh (`hero_after_refresh`), if the Tavern has room (7 cards), an exact
//! copy of the shop minion with the highest Tier (a tie at random: one `rng.below(k)`; engine
//! decision) joins the shop to its right (not from the pool), and both are frozen on their own
//! (`Unit::frozen`, `docs/tavern.md` §3.1): they stay through the next turn start.

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState, MAX_SHOP_SIZE};

pub const ID: HeroId = 79615;
pub const POWER: CardId = 80539;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_after_refresh(twice_as_nice))]
}

/// After a Refresh: copy the highest-Tier minion and freeze both.
fn twice_as_nice(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    if state.shop.len() >= MAX_SHOP_SIZE {
        return;
    }
    let Some(top) = state
        .shop
        .iter()
        .filter(|u| !u.is_spell)
        .map(|u| u.tavern_tier)
        .max()
    else {
        return;
    };
    let tied: Vec<usize> = (0..state.shop.len())
        .filter(|&i| !state.shop[i].is_spell && state.shop[i].tavern_tier == top)
        .collect();
    let pick = match tied.len() {
        1 => tied[0],
        k => tied[rng.below(k)],
    };
    state.shop[pick].frozen = true;
    let copy = state.shop[pick].clone();
    state.shop.insert(pick + 1, copy);
}
