//! `Writhing Tentacles` (`BG36_MagicItem_606`) — Greater, 0 Gold.
//!
//! After you discard your first minion each turn, get a copy of it with double stats. (1 left!)
//!
//! `trinket_after_discard`: the first minion discarded from the hand each turn (`armed`, which
//! starts again each turn) gives a copy of it (`Unit` as it was, a new copy: not from the pool)
//! with its Attack and Health doubled, which waits for room in a full hand.

use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134321;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_after_discard(after_discard),
    )]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_turn(state, ID);
}

fn after_discard(state: &mut TavernState, card: &Unit, _: &mut CardPool, _: &mut Rng) {
    if card.is_spell {
        return;
    }
    let Some(t) = super::state_mut(&mut state.auras, ID).filter(|t| t.armed == 0) else {
        return;
    };
    t.armed = 1;
    let mut copy = card.clone();
    copy.locked_turns = 0;
    copy.add_stats(copy.attack, copy.health);
    state.add_to_hand_or_wait(copy);
}
