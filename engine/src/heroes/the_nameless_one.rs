//! `The Nameless One` (`BGDUO_HERO_100`) — Duos Armor 12.
//!
//! Hero Power: `` (`BGDUO_HERO_100p`), 0 Gold: Start of Game: Copy your teammate's Hero Power.
//!
//! Duos only, passive. At the start of turn 1 in `DuosTavern::start_turn` (before either player's
//! `start_turn` runs), replaces `hero.power` with a fresh copy of the teammate's Hero Power and
//! runs its `hero_chosen` hook (`super::copy_teammate_power`).

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId, HeroState};

pub const ID: HeroId = 104567;
pub const POWER: CardId = 104569;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_turn_reset(noop))]
}

fn noop(_: &mut HeroState) {}
