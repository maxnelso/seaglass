//! `Tortollan Blue Shell` (`BG24_018`) — Tier 4 Neutral (`3/6`).
//!
//! If you lost your last combat, this minion sells for 5 (`10` if Golden) Gold.

use crate::cards::CardTemplate;
use crate::model::{CardId, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 457;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Tortollan Blue Shell", 3, 6, 4)
        .on_sell(|state, sold, _, _| on_sell(state, sold))
}

pub fn on_sell(state: &mut TavernState, sold: &Unit) {
    if state.last_combat_lost {
        let extra = if sold.is_golden { 9 } else { 4 };
        state.gold += extra;
    }
}
