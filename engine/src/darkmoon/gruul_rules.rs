//! `Gruul Rules` (`BGS_Treasures_009`) — Tier 2 Darkmoon Prize.
//!
//! Give a friendly minion "At the end of your turn, gain +4/+4."
//!
//! The target's `end_of_turn_stats` go up by 4 (`docs/darkmoon_prizes.md` §4.2): it gains +4/+4 at
//! each of its player's turn ends, with its end-of-turn triggers, Golden or not.

use crate::cards::spells::targeted;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 65246;

/// The stats it gives at each end of turn.
const STATS: i32 = 4;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, targeted(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, pos: usize, _: &mut CardPool, _: &mut Rng) {
    if let Some(unit) = state.board.get_mut(pos) {
        unit.end_of_turn_stats += STATS;
    }
}
