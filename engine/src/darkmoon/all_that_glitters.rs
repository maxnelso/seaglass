//! `All That Glitters` (`BGS_Treasures_037`) — Tier 3 Darkmoon Prize.
//!
//! Make a random minion in the Tavern Golden.
//!
//! As `Golden Touch`: a random non-Golden minion in the Tavern becomes Golden (`make_golden`: a
//! Golden an effect made, which still gives a Triple Reward when played).

use crate::cards::spells::{golden_touch, spell};
use crate::cards::CardHooks;
use crate::model::CardId;

pub const ID: CardId = 66947;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(golden_touch::cast))]
}
