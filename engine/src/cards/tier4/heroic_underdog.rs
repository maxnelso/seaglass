//! `Heroic Underdog` (`BG34_604`) — Tier 4 Neutral (`1/10`, Stealth).
//!
//! Stealth. Rally: Gain (`double` if Golden) the target's Attack.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Unit};

pub const ID: CardId = 428;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Heroic Underdog", 1, 10, 4).with_keyword(Keyword::Stealth)
}

pub fn on_rally(attacker: &mut Unit, target: Option<&Unit>) {
    if let Some(t) = target {
        let mult = if attacker.is_golden { 2 } else { 1 };
        let gain = (t.attack * mult).max(0);
        attacker.add_stats(gain, 0);
    }
}
