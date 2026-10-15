//! `Cho` (`BGDUO_HERO_222`) — Duos Armor 14.
//!
//! Hero Power: `Double Trouble` (`BGDUO_HERO_222p`), 0 Gold: Whenever you play a Golden minion, both you and Gall get a Triple Reward. (3 left!)
//!
//! Passive, Duos only. After a minion is played (`hero_after_play`): the first 3 Golden minions
//! played from the hand that give a Triple Reward (not `intrinsic_golden`; `stacks` counts them)
//! each give the player a `Triple Reward` card (if the hand has room) and their teammate one (a
//! teammate `Card` gift, whatever the teammate's hero: engine decision), on top of the Golden's own
//! Discover.

use crate::cards::{tokens, CardHooks, Played};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, TeammateGift};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 104981;
pub const POWER: CardId = 104983;

/// Golden minions it rewards.
const TIMES: i32 = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_after_play(after_play))]
}

/// After a Golden minion is played: a Triple Reward for the player and for Gall.
fn after_play(state: &mut TavernState, played: &Played, _: &mut CardPool, _: &mut Rng) {
    if !played.from_hand || played.magnetized {
        return;
    }
    let rewarding = state
        .board
        .get(played.board_pos)
        .is_some_and(|u| u.is_golden && !u.intrinsic_golden);
    let Some(power) = power_mut(&mut state.auras).filter(|p| rewarding && p.stacks < TIMES) else {
        return;
    };
    power.stacks += 1;
    state.add_to_hand(tokens::make_triple_reward());
    state
        .auras
        .give_teammate(TeammateGift::Card(tokens::make_triple_reward()));
}
