//! `Patches the Pirate` (`TB_BaconShop_HERO_18`) — Armor 19 (Duos 17).
//!
//! Hero Power: `Pirate Parrrrty!` (`TB_BaconShop_HP_072`), 3 Gold: Get a Pirate. After you buy a Pirate, your next Hero Power costs (1) less.
//!
//! Active, untargeted, once per turn: a random Pirate of the player's Tier or lower from the pool
//! (engine decision) goes to the hand; it needs room in the hand. Each Pirate bought
//! (`hero_after_buy`) makes the power 1 cheaper (never below 0); using it puts the cost back to 3.

use crate::cards::CardHooks;
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 57947;
pub const POWER: CardId = 62243;

/// Its full cost.
const COST: u32 = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(pirate_party)
            .on_hero_power_usable(super::hand_has_room)
            .on_hero_after_buy(after_buy),
    )]
}

/// Using it: a random Pirate; the cost is full again.
fn pirate_party(state: &mut TavernState, _: Option<usize>, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.cost = COST;
    }
    let tier = state.tavern_tier;
    if let Some(pirate) = pool
        .draw_discover_by_tribe(Tribe::Pirate, tier, 1, rng)
        .pop()
    {
        state.add_pool_card_to_hand(pirate, pool);
    }
}

/// After a Pirate is bought: 1 cheaper.
fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, _: &mut Rng) {
    if bought.is_spell || !bought.tribe.matches(Tribe::Pirate) {
        return;
    }
    if let Some(power) = power_mut(&mut state.auras) {
        power.cost = power.cost.saturating_sub(1);
    }
}
