//! `Guiding Candle` (`BG32_MagicItem_366`) — Greater, 1 Gold.
//!
//! Your first two Refreshes each turn contain only Tier 6 cards. (2 left!)
//!
//! `trinket_shop_tiers`: the cards the first 2 Refreshes the player does each turn (`counter`,
//! counted by `trinket_after_player_refresh` and starting again each turn; not the turn start's new
//! shop, as for "after you Refresh": **engine decision**) draw, minions and the spell, are of Tier 6
//! only (`TavernState::shop_tiers`).

use crate::cards::CardHooks;
use crate::model::{CardId, TrinketState};
use crate::rng::Rng;
use crate::tavern::{CardPool, RefreshKind, TavernState};

pub const ID: CardId = 120928;

/// Refreshes each turn that offer Tier 6 cards.
const REFRESHES: i32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_shop_tiers(tiers)
            .on_trinket_after_player_refresh(after_refresh),
    )]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_turn(state, ID);
}

fn tiers(trinket: &TrinketState, kind: RefreshKind, tiers: (u32, u32)) -> (u32, u32) {
    if kind == RefreshKind::Player && trinket.counter < REFRESHES {
        (6, 6)
    } else {
        tiers
    }
}

fn after_refresh(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        t.counter = (t.counter + 1).min(REFRESHES);
    }
}
