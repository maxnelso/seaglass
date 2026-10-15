//! `The Lich King` (`TB_BaconShop_HERO_22`) — Armor 14 (Duos 12).
//!
//! Hero Power: `Reborn Rites` (`TB_BaconShop_HP_024`), 0 Gold: Give a minion Reborn until next turn.
//!
//! Active, a friendly minion without Reborn (engine decision), once per turn: it gains Reborn
//! until next turn (`Unit::temp_reborn`, like `temp_divine_shield`: it ends at the start of the
//! next turn, unless a lasting Reborn was given since).

use crate::cards::{ActivateTargetKind, CardHooks};
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 58024;
pub const POWER: CardId = 58040;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(reborn_rites)
            .on_hero_power_target(|state, t| !state.board[t].reborn)
            .with_activate_target(ActivateTargetKind::BoardAny),
    )]
}

/// Using it: Reborn until next turn.
fn reborn_rites(state: &mut TavernState, target: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    if let Some(unit) = target.and_then(|t| state.board.get_mut(t)) {
        unit.reborn = true;
        unit.temp_reborn = true;
    }
}
