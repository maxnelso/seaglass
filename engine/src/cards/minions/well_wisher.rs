//! `Well Wisher` (`BGDUO_120`) — Tier 5 Neutral (`6/6`), Duos only.
//! **Spellcraft:** Pass a different non-Golden minion (Golden `BGDUO_120_G`, `12/12`: the same).
//!
//! Its Spellcraft spell is `Well Wishes` (Golden if it is: the same text), given when it is
//! played and at the start of each turn (`docs/duos_tavern.md` §8.3).

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Unit};

pub const ID: CardId = 10506;
pub const NAME: &str = "Well Wisher";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 6, 6, 5).on_spellcraft(spellcraft)
}

/// Its Spellcraft spell: `Well Wishes`, Golden if it is.
pub fn spellcraft(wisher: &Unit) -> Unit {
    let mut spell = spells::make_well_wishes();
    spell.is_golden = wisher.is_golden;
    spell
}
