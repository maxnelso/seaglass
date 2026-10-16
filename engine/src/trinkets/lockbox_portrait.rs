//! `Lockbox Portrait` (`BG36_MagicItem_301`) — Lesser, 1 Gold, Pirate.
//!
//! Get a Lockbox. If you already have one, it opens 2 turns sooner instead. At the start of each turn, repeat this.
//!
//! Picked, and at each turn start (after the Lockboxes in hand counted down): with a `Lockbox` in
//! the hand, the left-most one opens 2 turns sooner (`lockbox_turns_left`, down to 0: then it
//! opens at once, `lockbox::open_lockbox`); without one, a new `Lockbox` (5 turns) goes to the
//! hand, or waits for room in a full hand.

use crate::cards::spells::lockbox;
use crate::cards::tokens::{self, SPELL_LOCKBOX};
use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133341;

/// Turns sooner.
const SOONER: u32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(lockbox_or_sooner)
            .on_trinket_turn_start(lockbox_or_sooner),
    )]
}

fn lockbox_or_sooner(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    let Some(idx) = state.hand.iter().position(|u| u.card_id == SPELL_LOCKBOX) else {
        state.add_to_hand_or_wait(tokens::make_lockbox());
        return;
    };
    let lockbox = &mut state.hand[idx];
    lockbox.lockbox_turns_left = lockbox.lockbox_turns_left.saturating_sub(SOONER);
    if lockbox.lockbox_turns_left == 0 {
        lockbox::open_lockbox(state, idx, rng);
    }
}
