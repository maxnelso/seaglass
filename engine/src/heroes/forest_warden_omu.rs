//! `Forest Warden Omu` (`TB_BaconShop_HERO_74`) — Armor 6 (Duos 5).
//!
//! Hero Power: `Everbloom` (`TB_BaconShop_HP_082`), 0 Gold: After you upgrade the Tavern, gain 2 Gold.
//!
//! Passive. After each `UpgradeTavern` (`hero_after_upgrade`), the player gains 2 Gold.

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 63604;
pub const POWER: CardId = 63605;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_after_upgrade(after_upgrade))]
}

/// After an upgrade: 2 Gold.
fn after_upgrade(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.gold += 2;
}
