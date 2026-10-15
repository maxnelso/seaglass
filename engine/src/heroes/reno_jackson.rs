//! `Reno Jackson` (`TB_BaconShop_HERO_41`) — Armor 12 (Duos 10).
//!
//! Hero Power: `Gonna Be Rich!` (`TB_BaconShop_HP_046`), 0 Gold: Once per game, make a friendly minion Golden.
//!
//! Active, a friendly non-Golden minion, once per game (`stacks` is 1 once used): it becomes
//! Golden (`Unit::make_golden`, as `Eyes of the Earth Mother`), then the auras resync.

use crate::cards::{ActivateTargetKind, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 60212;
pub const POWER: CardId = 60216;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(gonna_be_rich)
            .on_hero_power_usable(super::unused_this_game)
            .on_hero_power_target(|state, t| !state.board[t].is_golden)
            .with_activate_target(ActivateTargetKind::BoardAny),
    )]
}

/// Using it: the minion becomes Golden, once per game.
fn gonna_be_rich(state: &mut TavernState, target: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.stacks = 1;
    }
    if let Some(unit) = target.and_then(|t| state.board.get_mut(t)) {
        unit.make_golden();
        state.sync_all_auras();
    }
}
