//! `Glowing Gauntlet` (`BG30_MagicItem_841`) — Lesser, 0 Gold, Demon.
//!
//! Minions in the Tavern have +3/+3. The Tavern always has 7 cards.
//!
//! The Tavern's bonus +3/+3 as picked (`buff_tavern_minions`). `trinket_shop_slots`: every shop
//! the Tavern rolls holds 7 cards, the spell slot among them (`TavernState::shop_minion_slots`).
//! `trinket_after_action`: after each action (its pick too), a shop with fewer cards (a card
//! bought, consumed, stolen) is filled back to 7 **(confirmed)**, as a turn start fills a kept
//! shop (`TavernState::top_up_shop`: the spell slot with a spell if no spell is left, the other
//! slots with minions, drawn as the turn start's shop draws them; not a Refresh: **engine
//! decision**).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState, MAX_SHOP_SIZE};

pub const ID: CardId = 111092;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_shop_slots(slots)
            .on_trinket_after_action(keep_full),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::buff_tavern_minions(state, 3, 3);
}

fn slots(state: &TavernState, _: usize) -> usize {
    MAX_SHOP_SIZE - usize::from(state.include_shop_spells)
}

fn keep_full(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if state.shop.len() < MAX_SHOP_SIZE {
        state.top_up_shop(pool, rng);
    }
}
