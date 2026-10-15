//! `Captain Eudora` (`TB_BaconShop_HERO_64`) — Armor 14 (Duos 12).
//!
//! Hero Power: `Buried Treasure` (`TB_BaconShop_HP_074`), 1 Gold: Dig for a Golden minion! (4 Digs left.)
//!
//! Active, untargeted, once per turn: each use is a Dig (`counter`); the 4th gives a Golden
//! minion of exactly the player's Tier from the pool (`Unit::make_golden`), and the count starts
//! again (engine decision). The 4th Dig needs room in the hand.

use crate::cards::CardHooks;
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 62242;
pub const POWER: CardId = 62250;

/// Digs per Golden minion.
const DIGS: i32 = 4;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(dig)
            .on_hero_power_usable(usable),
    )]
}

/// The Dig that finds the treasure needs room in the hand.
fn usable(state: &TavernState) -> bool {
    let digs = state.auras.hero.as_ref().map_or(0, |h| h.power.counter);
    digs + 1 < DIGS || super::hand_has_room(state)
}

/// Using it: a Dig; the 4th finds a Golden minion.
fn dig(state: &mut TavernState, _: Option<usize>, pool: &mut CardPool, rng: &mut Rng) {
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    power.counter += 1;
    if power.counter < DIGS {
        return;
    }
    power.counter = 0;
    if let Some(mut treasure) = pool.draw_discover_options(state.tavern_tier, 1, rng).pop() {
        treasure.make_golden();
        state.add_pool_card_to_hand(treasure, pool);
    }
}
