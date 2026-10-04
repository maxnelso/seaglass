//! `Twisted Wrathguard` (`BG35_155`) — Tier 6 Demon (`8/8`).
//!
//! After you sell a minion, add a (`2` if Golden) Fodder to your next Refresh.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 625;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Twisted Wrathguard", 8, 8, 6).with_tribe(Tribe::Demon)
}

pub fn after_sell_minion(state: &mut TavernState) {
    let mut count = 0u32;
    for u in &state.board {
        if u.card_id == ID {
            count += if u.is_golden { 2 } else { 1 };
        }
    }
    if count > 0 {
        state.auras.fodder_per_refresh[0] += count;
    }
}
