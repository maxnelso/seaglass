//! `Mysterybox` (`TB_BaconShop_HERO_35_Buddy_t7`) — Wheel of Yogg-Saron, 19%.
//!
//! Make a random minion in the Tavern Golden.
//!
//! As `Golden Touch` (`spells::golden_touch::cast`): a random non-Golden minion in the Tavern (one
//! `rng.below(k)`; spells don't count) becomes Golden (`make_golden`: a Golden made by an effect,
//! which still gives a Triple Reward when played); nothing without one (`docs/wheel_of_yogg.md`
//! §4).

use crate::cards::spells::{golden_touch, spell};
use crate::cards::CardHooks;
use crate::model::CardId;

pub const ID: CardId = 93390;

/// The outcome's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(golden_touch::cast))]
}
