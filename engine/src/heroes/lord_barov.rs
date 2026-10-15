//! `Lord Barov` (`TB_BaconShop_HERO_72`) — Armor 14 (Duos 12).
//!
//! Hero Power: `Friendly Wager` (`TB_BaconShop_HP_081`), 1 Gold: Guess which player will win their next combat. If you're correct, get 3 Tavern Coins.
//!
//! Active, targeted (`ActivateTargetKind::PairSide`: `target: Some(0)` for side A or `Some(1)` for
//! side B of `state.lobby.featured_pair`), once per turn when `state.lobby.featured_pair.is_some()`.
//! Stores the guess in `power.armed` (`1` for side A, `2` for side B). At the start of the next
//! turn (`hero_turn_start`), checks `state.lobby.last_pair_outcome`: if the guessed side won, adds
//! 3 `Tavern Coin`s to hand; if `BattleOutcome::Draw`, refunds 1 `Tavern Coin`; otherwise 0.

use crate::cards::{tokens, ActivateTargetKind, CardHooks};
use crate::heroes::power_mut;
use crate::model::{BattleOutcome, CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 63602;
pub const POWER: CardId = 63600;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .with_activate_target(ActivateTargetKind::PairSide)
            .on_hero_power(place_wager)
            .on_hero_power_usable(|state| state.lobby.featured_pair.is_some())
            .on_hero_turn_start(settle_wager),
    )]
}

/// Using it: record the wagered side (`1` for `Some(0)`, `2` for `Some(1)`).
fn place_wager(state: &mut TavernState, target: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.armed = (target.unwrap_or(0) + 1) as u32;
    }
}

/// Start of turn: pay out 3 `Tavern Coin`s on a winning guess, or 1 on a tie.
fn settle_wager(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let Some(guess) = power_mut(&mut state.auras)
        .map(|p| std::mem::take(&mut p.armed))
        .filter(|&g| g > 0)
    else {
        return;
    };
    let coins = match (guess, state.lobby.last_pair_outcome) {
        (1, Some(BattleOutcome::AWin)) | (2, Some(BattleOutcome::BWin)) => 3,
        (_, Some(BattleOutcome::Draw)) => 1,
        _ => 0,
    };
    for _ in 0..coins {
        state.add_to_hand(tokens::make_tavern_coin());
    }
}
