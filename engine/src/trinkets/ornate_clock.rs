//! `Ornate Clock` (`BG32_MagicItem_271`) — Lesser, 0 Gold.
//!
//! Gain 2 Gold. Buy your Greater Trinket next turn instead of Turn 9.
//!
//! Picked: 2 Gold, and it remembers the turn (`counter`). `trinket_offer_kind`
//! (`docs/trinkets.md` §3.5): the turn after that one has the Greater offer, and turn 9 none;
//! picked on turn 8 or later, nothing changes **(engine decision)**.

use crate::cards::CardHooks;
use crate::model::{CardId, TrinketState};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 121120;

/// The Gold it gives.
const GOLD: u32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_offer_kind(offer_kind),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.gold += GOLD;
    let turn = state.turn as i32;
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        t.counter = turn;
    }
}

fn offer_kind(t: &TrinketState, state: &TavernState, kind: Option<bool>) -> Option<bool> {
    let early = t.counter as u32 + 1;
    if early >= super::GREATER_TURN {
        return kind;
    }
    if state.turn == early {
        Some(true)
    } else if state.turn == super::GREATER_TURN {
        None
    } else {
        kind
    }
}
