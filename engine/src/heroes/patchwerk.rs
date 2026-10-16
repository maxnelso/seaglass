//! `Patchwerk` (`TB_BaconShop_HERO_34`) — Armor 0 (Duos 0).
//!
//! Hero Power: `All Patched Up` (`TB_BaconShop_HP_035`), 0 Gold: Start the game with 30 extra Health.
//!
//! Passive: the hero data gives Patchwerk 60 Health and no Armor (`docs/heroes.md` §2.3,
//! confirmed); in Duos his extra 30 Health goes into the team's pool. Another hero who gets the
//! Hero Power (Sir Finley's or Genn's Discover, The Nameless One's copy) gains the 30 Health then
//! (`hero_chosen`, engine decision).
//!
//! Master Nguyen never offers it for a single turn (`CardFlags::NOT_A_TURN_POWER`): its 30 extra
//! Health would come back every turn.

use crate::cards::{CardFlags, CardHooks};
use crate::model::{CardId, HeroId};
use crate::tavern::TavernState;

/// The extra Health.
const EXTRA_HEALTH: i32 = 30;

pub const ID: HeroId = 59397;
pub const POWER: CardId = 59399;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .with_flags(CardFlags::NOT_A_TURN_POWER)
            .on_hero_chosen(extra_health),
    )]
}

/// Another hero got the Hero Power: 30 extra Health (Patchwerk's own is in his data).
fn extra_health(state: &mut TavernState) {
    if state.hero().is_some_and(|h| h.hero_id != ID) {
        state.health += EXTRA_HEALTH;
    }
}
