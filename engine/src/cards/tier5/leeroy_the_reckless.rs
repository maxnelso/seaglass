//! `Leeroy the Reckless` (`BG23_318`) — Tier 5 Neutral (`6/2`).
//!
//! Deathrattle: Destroy the minion that killed this.

use crate::cards::CardTemplate;
use crate::model::CardId;

pub const ID: CardId = 529;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Leeroy the Reckless", 6, 2, 5)
}
