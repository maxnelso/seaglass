//! `Captain Hooktusk` (`TB_BaconShop_HERO_67`) — Armor 14 (Duos 12).
//!
//! Hero Power: `Trash for Treasure` (`TB_BaconShop_HP_075`), 0 Gold: Remove a friendly minion. Discover one from a Tier lower to get.
//!
//! Active, a friendly minion, once per turn: the minion leaves the board (its copies return to the
//! pool; not a sale or a death), then a Discover among 3 minions of exactly its Tier minus 1 (Tier
//! 1 for a Tier 1 minion: engine decision) from the pool. It needs room in the hand.

use crate::cards::{ActivateTargetKind, CardHooks};
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 62266;
pub const POWER: CardId = 62267;

/// Discover options.
const OPTIONS: usize = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(trash_for_treasure)
            .on_hero_power_usable(super::hand_has_room)
            .with_activate_target(ActivateTargetKind::BoardAny),
    )]
}

/// Using it: remove the minion, Discover one from a Tier lower.
fn trash_for_treasure(
    state: &mut TavernState,
    target: Option<usize>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let Some(t) = target.filter(|&t| t < state.board.len()) else {
        return;
    };
    let removed = state.board.remove(t);
    pool.return_unit(&removed);
    state.sync_all_auras();
    let tier = removed.tavern_tier.saturating_sub(1).max(1);
    let options = pool.draw_discover_options(tier, OPTIONS, rng);
    state.push_discover(options);
}
