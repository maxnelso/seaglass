//! `Patchwerk` (`TB_BaconShop_HERO_34`) — Armor 0 (Duos 0).
//!
//! Hero Power: `All Patched Up` (`TB_BaconShop_HP_035`), 0 Gold: Start the game with 30 extra Health.
//!
//! Passive, with no hooks: the hero data gives Patchwerk 60 Health and no Armor
//! (`docs/heroes.md` §2.3, confirmed); in Duos his extra 30 Health goes into the team's pool.

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId};

pub const ID: HeroId = 59397;
pub const POWER: CardId = 59399;

/// The Hero Power's (empty) hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY)]
}
