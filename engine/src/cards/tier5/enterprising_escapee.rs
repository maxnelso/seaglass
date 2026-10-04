//! `Enterprising Escapee` (`BG36_523`) — Tier 5 Pirate (`6/6`).
//!
//! After you spend 6 Gold, get a `Lockbox`. If you already have one, it opens `1` (`2` if Golden) turn(s) sooner instead.

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 515;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Enterprising Escapee", 6, 6, 5).with_tribe(Tribe::Pirate)
}

pub fn on_gold_spent(state: &mut TavernState, amount: u32, rng: &mut Rng) {
    let mut triggers: Vec<u32> = Vec::new();
    for u in &mut state.board {
        if u.card_id == ID {
            u.gunpowder_gold_progress += amount;
            let n = u.gunpowder_gold_progress / 6;
            u.gunpowder_gold_progress %= 6;
            let reduction = if u.is_golden { 2 } else { 1 };
            for _ in 0..n {
                triggers.push(reduction);
            }
        }
    }
    for reduction in triggers {
        if let Some(lockbox) = state
            .hand
            .iter_mut()
            .find(|h| h.card_id == tokens::SPELL_LOCKBOX && h.lockbox_turns_left > 0)
        {
            lockbox.lockbox_turns_left = lockbox.lockbox_turns_left.saturating_sub(reduction);
            state.open_ready_lockboxes(rng);
        } else {
            state.add_to_hand(tokens::make_lockbox());
        }
    }
}
