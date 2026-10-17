//! `Trip Vouchers` (`BG30_MagicItem_891`) — Lesser, 1 Gold.
//!
//! In 2 turns, choose a Greater Trinket to buy and replace this. (2 turns left!)
//!
//! At the 2nd turn start after it is picked (`counter`; "Next turn!" after the first): a Greater
//! offer drawn as on turn 9 (`trinkets::draw_offer`, its prices), each option replacing the
//! Vouchers (`docs/trinkets.md` §3.5); the turn 9 offer still comes **(engine decision)**.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 112991;

/// Turn starts until the offer.
const TURNS: i32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_turn_start(new_turn))]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    let Some(t) = super::state_mut(&mut state.auras, ID) else {
        return;
    };
    t.counter += 1;
    if t.counter != TURNS {
        return;
    }
    let mut options = super::draw_offer(state, true, rng);
    for option in &mut options {
        option.replaces = Some(ID);
    }
    super::push_offer(state, options);
}
