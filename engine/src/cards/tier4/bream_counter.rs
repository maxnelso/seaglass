//! `Bream Counter` (`BG26_137`) — Tier 4 Murloc (`6/6`).
//!
//! While this is in your hand, after you play a Murloc, gain `+6/+6` (`+12/+12` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 409;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Bream Counter", 6, 6, 4).with_tribe(Tribe::Murloc)
}

pub fn after_play_minion(state: &mut TavernState, played_tribe: Tribe) {
    if !played_tribe.matches(Tribe::Murloc) {
        return;
    }
    for h in &mut state.hand {
        if h.card_id == ID {
            let buff = if h.is_golden { 12 } else { 6 };
            h.add_stats(buff, buff);
        }
    }
}
