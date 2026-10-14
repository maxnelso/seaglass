//! `Wanderer Cho` (`BGDUO_100`) — Tier 2 Neutral (`4/3`), Duos only.
//! One Pass each turn is free (Golden `BGDUO_100_G`, `8/6`: Two Passes each turn are free).
//!
//! A Pass is free while a [`CardFlags::FREE_PASS`] minion has `charges` left, using one of them
//! (`docs/duos_tavern.md` §5.2): this has 1 (2 if Golden) each turn.

use crate::cards::{CardFlags, CardTemplate};
use crate::model::{CardId, Unit};

pub const ID: CardId = 10201;
pub const NAME: &str = "Wanderer Cho";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 3, 2)
        .with_flags(CardFlags::FREE_PASS)
        .on_reset_turn_charges(reset_turn_charges)
        .on_made_golden(|u| u.charges += 1)
}

/// Its free Passes this turn: 1, or 2 if Golden.
pub fn reset_turn_charges(unit: &mut Unit) {
    unit.charges = if unit.is_golden { 2 } else { 1 };
}
