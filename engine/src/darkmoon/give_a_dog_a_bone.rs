//! `Give a Dog a Bone` (`BGS_Treasures_028`) — Tier 4 Darkmoon Prize.
//!
//! Give a friendly minion Divine Shield, Windfury, and +15/+15.

use crate::cards::spells::targeted;
use crate::cards::CardHooks;
use crate::model::{CardId, Keyword, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 66424;

/// The stats it gives.
const STATS: i32 = 15;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, targeted(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, pos: usize, _: &mut CardPool, _: &mut Rng) {
    if let Some(unit) = state.board.get_mut(pos) {
        unit.apply_keyword(Keyword::DivineShield, false);
        unit.apply_keyword(Keyword::Windfury, false);
        unit.add_stats(STATS, STATS);
    }
}
