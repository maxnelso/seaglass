//! `Mysterious Orb` (`BG35_MagicItem_818`) — Lesser, 0 Gold.
//!
//! Gain 10 Gold. Your next Trinket is a Lesser Trinket.
//!
//! Picked: 10 Gold, and it remembers the turn (`counter`). `trinket_offer_kind`
//! (`docs/trinkets.md` §3.5): the first scheduled offer after that turn (the turn 9 one) is a
//! Lesser offer; picked on turn 9 or later, nothing changes.

use crate::cards::CardHooks;
use crate::model::{CardId, TrinketState};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130836;

/// The Gold it gives.
const GOLD: u32 = 10;

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
    let next_scheduled = state.turn > t.counter as u32
        && (t.counter as u32 + 1..state.turn)
            .all(|turn| turn != super::LESSER_TURN && turn != super::GREATER_TURN);
    match kind {
        Some(_) if next_scheduled => Some(false),
        _ => kind,
    }
}
