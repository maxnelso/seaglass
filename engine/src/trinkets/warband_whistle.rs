//! `Warband Whistle` (`BG35_MagicItem_930`) — Greater, 2 Gold.
//!
//! Gain a free Refresh that contains plain copies of minions in your warband.
//!
//! A free Refresh as picked (`free_refreshes`, which carries over) and a charge (`armed`).
//! `trinket_refresh`: the player's next `Refresh` action, with the charge left, is the Refresh of
//! plain copies of the board's minions (`TavernState::refresh_shop_with_warband`, as Tess
//! Greymane's: up to the shop's minion slots, chosen at random beyond them) **(engine decision)**.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131002;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_refresh(refresh),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.auras.free_refreshes += 1;
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        t.armed += 1;
    }
}

fn refresh(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) -> bool {
    match super::state_mut(&mut state.auras, ID) {
        Some(t) if t.armed > 0 => t.armed -= 1,
        _ => return false,
    }
    let warband = state.board.clone();
    state.refresh_shop_with_warband(&warband, pool, rng);
    true
}
