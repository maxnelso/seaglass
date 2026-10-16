//! `Transcribing Typewriter` (`BG35_MagicItem_931`) — Lesser, 1 Gold.
//!
//! Get an extra copy of the next 2 minions you buy. (2 left!)
//!
//! Picked, it has 2 copies left (`counter`). `trinket_after_buy`: each minion bought while some
//! are left gives a plain copy of it (`cards::plain_instance`, not from the pool: **engine
//! decision**) to the hand, or waiting for room in a full hand (`docs/trinkets.md` §5); the copy
//! can complete a Triple.

use crate::cards::{self, CardHooks};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131003;

/// Copies it gives.
const COPIES: i32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_after_buy(after_buy),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        t.counter = COPIES;
    }
}

fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, _: &mut Rng) {
    if bought.is_spell {
        return;
    }
    let Some(t) = super::state_mut(&mut state.auras, ID).filter(|t| t.counter > 0) else {
        return;
    };
    t.counter -= 1;
    if let Some(copy) = cards::plain_instance(bought) {
        state.add_to_hand_or_wait(copy);
    }
}
