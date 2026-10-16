//! `The Nameless One` (`BGDUO_HERO_100`) — Duos Armor 12.
//!
//! Hero Power: `` (`BGDUO_HERO_100p`), 0 Gold: Start of Game: Copy your teammate's Hero Power.
//!
//! Duos only, passive. Its `hero_before_team_turn` hook (`DuosTavern::start_turn`, before either
//! player's `start_turn` runs) replaces `hero.power` with a fresh copy of the teammate's hero's
//! Hero Power and runs its `hero_chosen` hook. The copy has no such hook: it happens once, at the
//! start of the team's first turn.
//!
//! Never offered by a Discover of Hero Powers (`CardFlags::NOT_DISCOVERABLE_POWER`): it copies a
//! Hero Power itself.

use crate::cards::{hooks, CardFlags, CardHooks};
use crate::model::{CardId, HeroId};
use crate::tavern::TavernState;

pub const ID: HeroId = 104567;
pub const POWER: CardId = 104569;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .with_flags(CardFlags::NOT_DISCOVERABLE_POWER)
            .on_hero_before_team_turn(copy_teammate_power),
    )]
}

/// Start of the game: a fresh copy of the teammate's hero's Hero Power replaces this one.
fn copy_teammate_power(state: &mut TavernState, mate: &TavernState) {
    let Some(mate_hero) = mate.hero().map(|h| h.hero_id) else {
        return;
    };
    assert_ne!(
        mate_hero, ID,
        "a Duos team cannot have two copies of The Nameless One"
    );
    let Some(mate_data) = super::data(mate_hero) else {
        return;
    };
    if let Some(hero) = state.auras.hero.as_mut() {
        hero.power = super::new_power_state(mate_data.power);
    }
    if let Some(chosen) = hooks(mate_data.power.id).hero_chosen {
        chosen(state);
    }
    state.sync_all_auras();
}
