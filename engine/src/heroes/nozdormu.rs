//! `Nozdormu` (`TB_BaconShop_HERO_57`) — Armor 13 (Duos 10).
//!
//! Hero Power: `Clairvoyance` (`TB_BaconShop_HP_063`), 0 Gold: At the start of your turn, gain a free Refresh.
//!
//! Passive. At the start of each turn, once the shop is filled (`hero_turn_start`), the player
//! gains a free Refresh (`free_refreshes`; an unused one carries over, as other free Refreshes do:
//! engine decision).

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 61489;
pub const POWER: CardId = 61491;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_turn_start(turn_start))]
}

/// Start of turn: a free Refresh.
fn turn_start(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.auras.free_refreshes += 1;
}
