//! `Trade Prince Gallywix` (`TB_BaconShop_HERO_10`) — Armor 5 (Duos 3).
//!
//! Hero Power: `Smart Savings` (`TB_BaconShop_HP_008`), 1 Gold: After you sell a minion, gain 1 Gold next turn.
//!
//! Passive (the data's cost is never paid). After the player sells a minion (`hero_after_sell`),
//! they gain 1 Gold next turn (`bonus_gold_next_turn`).

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 57891;
pub const POWER: CardId = 57559;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_after_sell(after_sell))]
}

/// After a sale: 1 Gold next turn.
fn after_sell(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    state.bonus_gold_next_turn += 1;
}
