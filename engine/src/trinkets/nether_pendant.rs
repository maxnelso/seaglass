//! `Nether Pendant` (`BG30_MagicItem_541`) — Greater, 5 Gold, Demon.
//!
//! Minions in the Tavern have +2/+2. Improves every 3 times your hero takes damage outside combat. (3 left!)
//!
//! Picked: the Tavern's bonus (`auras.tavern_all_atk` / `tavern_all_hp`, as `Staff of
//! Enrichment`'s) goes up by +2/+2, the minions in the Tavern now too. `trinket_hero_damage`:
//! every 3rd time the hero takes damage in the Tavern (damage a minion rewinds doesn't count;
//! `counter`, across turns), it improves by +2/+2 more (**engine decision**).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 112382;

/// Hero damage per improvement.
const TIMES: i32 = 3;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_hero_damage(hero_damage),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::buff_tavern_minions(state, 2, 2);
}

fn hero_damage(state: &mut TavernState, _: i32) {
    if super::count_to(&mut state.auras, ID, TIMES) {
        super::buff_tavern_minions(state, 2, 2);
    }
}
