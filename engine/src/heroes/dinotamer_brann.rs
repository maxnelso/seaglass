//! `Dinotamer Brann` (`TB_BaconShop_HERO_43`) — Armor 18 (Duos 16).
//!
//! Hero Power: `Battle Brand` (`TB_BaconShop_HP_048`), 0 Gold: After you buy 4 Battlecry minions, get a Brann Bronzebeard. (Once per game.)
//!
//! Passive. Every bought minion with a Battlecry counts (`hero_after_buy`, `counter`, across
//! turns); the 4th gives the player a `Brann Bronzebeard` (a plain one from the catalog, not the
//! pool: engine decision), once per game (`stacks` is 1 once given).

use crate::cards::{self, minions::brann_bronzebeard, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 60214;
pub const POWER: CardId = 60218;

/// Battlecry minions to buy.
const BUYS: i32 = 4;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_after_buy(after_buy))]
}

/// After a Battlecry minion is bought: the 4th gives a Brann, once per game.
fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, _: &mut Rng) {
    if bought.is_spell || cards::hooks(bought.card_id).battlecry.is_none() {
        return;
    }
    let Some(power) = power_mut(&mut state.auras).filter(|p| p.stacks == 0) else {
        return;
    };
    power.counter += 1;
    if power.counter < BUYS {
        return;
    }
    power.stacks = 1;
    if let Some(brann) = cards::template(brann_bronzebeard::ID) {
        state.add_to_hand(brann.instantiate());
    }
}
