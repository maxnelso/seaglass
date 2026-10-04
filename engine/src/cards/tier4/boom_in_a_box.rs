//! `Boom-in-a-Box` (`BG36_620`) — Tier 4 Neutral (`5/10`, Taunt).
//!
//! Taunt. Start of Combat: Deal 3 damage to all other minions (`twice` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword};

pub const ID: CardId = 407;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Boom-in-a-Box", 5, 10, 4).with_keyword(Keyword::Taunt)
}
