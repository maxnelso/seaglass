//! `Enterprising Escapee` (`BG36_523`) — Tier 5 Pirate (`6/6`).
//!
//! After you spend 6 Gold, get a `Lockbox`. If you already have one, it opens `1` (`2` if Golden) turn(s) sooner instead.

use crate::cards::{self, tokens, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 515;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Enterprising Escapee", 6, 6, 5)
        .with_tribe(Tribe::Pirate)
        .on_gold_spent(on_gold_spent)
}

pub fn on_gold_spent(
    state: &mut TavernState,
    self_idx: usize,
    amount: u32,
    _: &mut CardPool,
    rng: &mut Rng,
) {
    let escapee = &mut state.board[self_idx];
    escapee.counter += amount as i32;
    let triggers = escapee.counter / 6;
    escapee.counter %= 6;
    let reduction = escapee.golden_mult() as u32;
    for _ in 0..triggers {
        if let Some(lockbox) = state
            .hand
            .iter_mut()
            .find(|h| h.card_id == tokens::SPELL_LOCKBOX && h.lockbox_turns_left > 0)
        {
            lockbox.lockbox_turns_left = lockbox.lockbox_turns_left.saturating_sub(reduction);
            cards::resolve_ready_hand_cards(state, rng);
        } else {
            state.add_to_hand(tokens::make_lockbox());
        }
    }
}
