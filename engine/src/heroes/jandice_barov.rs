//! `Jandice Barov` (`TB_BaconShop_HERO_71`) — Armor 18 (Duos 15).
//!
//! Hero Power: `Swap, Lock, & Shop It` (`TB_BaconShop_HP_084`), 0 Gold: Swap a friendly non-Golden minion with a random one in the Tavern.
//!
//! Active, a friendly non-Golden minion, once per turn: a random minion in the shop (one
//! `rng.below(k)`) takes the friendly minion's board slot, and the friendly minion takes its shop
//! slot, each as it is (not a buy, a sale or a summon). It needs a minion in the shop.

use crate::cards::{ActivateTargetKind, CardHooks};
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 63601;
pub const POWER: CardId = 63607;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(swap)
            .on_hero_power_usable(|state| state.shop.iter().any(|u| !u.is_spell))
            .on_hero_power_target(|state, t| !state.board[t].is_golden)
            .with_activate_target(ActivateTargetKind::BoardAny),
    )]
}

/// Using it: swap the minion with a random shop minion.
fn swap(state: &mut TavernState, target: Option<usize>, _: &mut CardPool, rng: &mut Rng) {
    let Some(t) = target.filter(|&t| t < state.board.len()) else {
        return;
    };
    let minions: Vec<usize> = (0..state.shop.len())
        .filter(|&i| !state.shop[i].is_spell)
        .collect();
    let pick = match minions.len() {
        0 => return,
        1 => minions[0],
        k => minions[rng.below(k)],
    };
    std::mem::swap(&mut state.board[t], &mut state.shop[pick]);
    state.sync_all_auras();
}
