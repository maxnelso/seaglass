//! `Sous Chef Sticker` (`BG35_MagicItem_801`) — Lesser, 2 Gold.
//!
//! You can use your Hero Power an extra time each turn. After you use your Hero Power, gain (1) Gold.
//!
//! `trinket_hero_power_uses`: an active Hero Power (each of the hero's, the second one too) has a
//! second use each turn; picked, the uses left this turn go up by 1 at once **(engine decision)**.
//! `trinket_after_hero_power`: each use gives 1 Gold, once its cost is paid.

use crate::cards::CardHooks;
use crate::heroes;
use crate::model::{CardId, TrinketState};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130028;

/// The Gold each use gives.
const GOLD: u32 = 1;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_hero_power_uses(extra_uses)
            .on_trinket_after_hero_power(after_hero_power),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    if let Some(hero) = state.auras.hero.as_mut() {
        for power in std::iter::once(&mut hero.power).chain(hero.second_power.as_mut()) {
            if heroes::is_active(power.card_id) {
                power.uses_left += 1;
            }
        }
    }
}

fn extra_uses(_: &TrinketState) -> u32 {
    1
}

fn after_hero_power(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.gold += GOLD;
}
