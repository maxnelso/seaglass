//! `Bream Counter` (`BG26_137`) — Tier 4 Murloc (`6/6`).
//!
//! While this is in your hand, after you play a Murloc, gain `+6/+6` (`+12/+12` if Golden).

use crate::cards::{CardTemplate, Played};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 409;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Bream Counter", 6, 6, 4)
        .with_tribe(Tribe::Murloc)
        .on_after_friendly_play_in_hand(after_friendly_play_in_hand)
}

/// While in hand: after you play a Murloc, gain +6/+6 (+12/+12 if Golden).
pub fn after_friendly_play_in_hand(unit: &mut Unit, played: &Played) {
    if played.magnetized || !played.tribe.matches(Tribe::Murloc) {
        return;
    }
    let buff = if unit.is_golden { 12 } else { 6 };
    unit.add_stats(buff, buff);
}
