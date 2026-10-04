//! `Sin'dorei Straight Shot` (`BG25_016`) — Tier 4 Neutral (`3/4`, Divine Shield, Windfury).
//!
//! Divine Shield, Windfury. Rally: Remove Reborn and Taunt from the target.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Unit};

pub const ID: CardId = 451;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Sin'dorei Straight Shot", 3, 4, 4)
        .with_keyword(Keyword::DivineShield)
        .with_keyword(Keyword::Windfury)
}

pub fn on_rally(target: Option<&mut Unit>) {
    if let Some(t) = target {
        t.reborn = false;
        t.taunt = false;
    }
}
