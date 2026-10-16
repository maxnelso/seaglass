//! `Glowing Gauntlet` (`BG30_MagicItem_841`) — Lesser, 0 Gold, Demon.
//!
//! Minions in the Tavern have +3/+3. The Tavern always has 7 cards.
//!
//! The Tavern's bonus +3/+3 as picked (`buff_tavern_minions`). `trinket_shop_slots`: every shop
//! the Tavern rolls holds 7 cards, the spell slot among them (`TavernState::shop_minion_slots`);
//! a shop with cards bought from it isn't filled again, and the shop as it is when picked stays
//! **(engine decision)**.

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
            .on_trinket_shop_slots(slots),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::buff_tavern_minions(state, 3, 3);
}

fn slots(state: &TavernState, _: usize) -> usize {
    MAX_SHOP_SIZE - usize::from(state.include_shop_spells)
}
