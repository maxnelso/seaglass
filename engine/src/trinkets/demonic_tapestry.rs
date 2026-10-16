//! `Demonic Tapestry` (`BG35_MagicItem_152`) — Lesser, 2 Gold, Demon.
//!
//! After you Refresh 4 times, the highest-Tier minion in the Tavern costs Health instead of Gold to buy. (4 left!)
//!
//! `trinket_after_player_refresh`: every 4th Refresh the player does (`counter`, across turns;
//! **engine decision**: it repeats) marks the minion of the highest Tier in the Tavern (ties broken
//! at random: confirmed rule) as costing its price in Health instead of Gold (`Unit::costs_health`,
//! read by `TavernState::buy_price`), until it leaves the Tavern.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131280;

/// Refreshes per mark.
const REFRESHES: i32 = 4;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_after_player_refresh(after_refresh),
    )]
}

fn after_refresh(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    if !super::count_to(&mut state.auras, ID, REFRESHES) {
        return;
    }
    let Some(best) = state
        .shop
        .iter()
        .filter(|u| !u.is_spell)
        .map(|u| u.tavern_tier)
        .max()
    else {
        return;
    };
    let tied: Vec<usize> = (0..state.shop.len())
        .filter(|&i| !state.shop[i].is_spell && state.shop[i].tavern_tier == best)
        .collect();
    let pick = super::pick_tied(&tied, rng);
    state.shop[pick].costs_health = true;
}
