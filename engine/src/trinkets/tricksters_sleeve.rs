//! `Trickster's Sleeve` (`BG36_MagicItem_609`) — Lesser, 1 Gold.
//!
//! After you buy a minion, gain 1 Gold. (Then swap to Tavern spell!)
//!
//! `trinket_after_buy`: it waits for a minion bought (`armed` 0), then for a Tavern spell bought
//! (`armed` 1), and so on: each buy of the awaited kind gives 1 Gold and swaps it.

use crate::cards::{spells, CardHooks};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134642;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_buy(after_buy))]
}

fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, _: &mut Rng) {
    let is_tavern_spell = bought.is_spell && spells::is_tavern_spell(bought.card_id);
    let Some(t) = super::state_mut(&mut state.auras, ID) else {
        return;
    };
    let awaited = if t.armed == 0 {
        !bought.is_spell
    } else {
        is_tavern_spell
    };
    if awaited {
        t.armed = 1 - t.armed;
        state.gold += 1;
    }
}
