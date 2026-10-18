//! `All-Seeing Elder` (`BG33_300`) — Tier 4 Darkmoon Prize.
//!
//! Start of Combat: Gain the Attack of your left-most minion and the Health of your right-most minion.
//!
//! The minion prize: a minion in no pool, `cards::minions::all_seeing_elder`, whose hooks are its
//! template's; a Tier 4 prize Discover offers it like the prize spells.

use crate::cards::CardHooks;
use crate::model::CardId;

pub const ID: CardId = 122104;

/// The prize's own hooks: none (the minion's are its template's).
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    Vec::new()
}
