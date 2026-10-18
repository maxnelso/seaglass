//! `The Bouncer` (`BGS_Treasures_026`) — Tier 2 Darkmoon Prize.
//!
//! Give a friendly minion Taunt, then double its Health.
//!
//! Taunt (a granted Taunt, which a conditional Taunt doesn't take away), then it gains its Health.

use crate::cards::spells::targeted;
use crate::cards::CardHooks;
use crate::model::{CardId, Keyword, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 65380;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, targeted(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, pos: usize, _: &mut CardPool, _: &mut Rng) {
    if let Some(unit) = state.board.get_mut(pos) {
        unit.apply_keyword(Keyword::Taunt, false);
        let health = unit.health;
        unit.add_stats(0, health);
    }
}
