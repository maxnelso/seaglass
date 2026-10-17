//! `Sinstone Sticker` (`BG30_MagicItem_801`) — Greater, 3 Gold.
//!
//! The first 2 times you Discover a card each turn, get an extra copy of it. (2 left!)
//!
//! `trinket_after_discover`: the first 2 Discovers resolved each turn (`counter`, which starts
//! again each turn; any Discover, a Triple Reward's too, not a Choose One) give a copy of the
//! chosen card as it was offered (a new copy, not from the pool).

use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 110928;

/// Discovers each turn that give a copy.
const TIMES: i32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_after_discover(discovered),
    )]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_turn(state, ID);
}

fn discovered(state: &mut TavernState, chosen: &Unit, _: &[CardId], _: &mut CardPool, _: &mut Rng) {
    let Some(t) = super::state_mut(&mut state.auras, ID) else {
        return;
    };
    if t.counter >= TIMES {
        return;
    }
    t.counter += 1;
    state.add_to_hand_or_wait(chosen.clone());
}
