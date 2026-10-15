//! `George the Fallen` (`TB_BaconShop_HERO_15`) — Armor 15 (Duos 12).
//!
//! Hero Power: `Boon of Light` (`TB_BaconShop_HP_010`), 1 Gold: Give a minion Divine Shield.
//!
//! Active, a friendly minion, once per turn: it gains Divine Shield. A minion that has it is not
//! a target (engine decision).

use crate::cards::{ActivateTargetKind, CardHooks};
use crate::model::{CardId, HeroId, Keyword};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 57929;
pub const POWER: CardId = 57562;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(boon_of_light)
            .on_hero_power_target(|state, t| !state.board[t].divine_shield)
            .with_activate_target(ActivateTargetKind::BoardAny),
    )]
}

/// Using it: Divine Shield.
fn boon_of_light(state: &mut TavernState, target: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    if let Some(unit) = target.and_then(|t| state.board.get_mut(t)) {
        unit.apply_keyword(Keyword::DivineShield, false);
    }
}
