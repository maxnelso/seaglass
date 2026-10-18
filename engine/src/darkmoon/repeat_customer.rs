//! `Repeat Customer` (`BGS_Treasures_034`) — Tier 3 Darkmoon Prize.
//!
//! Return a friendly non-Golden minion to your hand. Give it +6/+6.
//!
//! A friendly non-Golden minion gains +6/+6, then leaves the board for the hand (not a sale, not
//! a death: its copies stay with it), where it can complete a Triple (engine decision: the buff
//! first). With a full hand (a prize a card casts), it stays on the board with the +6/+6 (engine
//! decision).

use crate::cards::spells::targeted;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 66495;

/// The stats it gives.
const STATS: i32 = 6;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        targeted(cast).on_spell_target_filter(|unit| !unit.is_golden),
    )]
}

fn cast(state: &mut TavernState, _: &Unit, pos: usize, _: &mut CardPool, _: &mut Rng) {
    if let Some(unit) = state.board.get_mut(pos) {
        unit.add_stats(STATS, STATS);
        super::return_to_hand(state, pos);
    }
}
