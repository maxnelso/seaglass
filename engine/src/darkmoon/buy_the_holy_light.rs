//! `Buy the Holy Light` (`BGS_Treasures_015`) — Tier 3 Darkmoon Prize.
//!
//! Give a friendly minion +10 Attack and Divine Shield.

use crate::cards::spells::targeted;
use crate::cards::CardHooks;
use crate::model::{CardId, Keyword, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 65259;

/// The Attack it gives.
const ATTACK: i32 = 10;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, targeted(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, pos: usize, _: &mut CardPool, _: &mut Rng) {
    if let Some(unit) = state.board.get_mut(pos) {
        unit.add_stats(ATTACK, 0);
        unit.apply_keyword(Keyword::DivineShield, false);
    }
}
