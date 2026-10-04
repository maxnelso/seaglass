//! `Hasty Excavation` — Tier 2 Tavern spell (`3` Health).
//!
//! Gain 1 Gold (cast like `Tavern Coin`).

use super::{spell, tavern_coin};
use crate::cards::CardHooks;

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(tavern_coin::cast)
}
