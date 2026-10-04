//! `Blue Whelp` (`BG33_924`) — Tier 3 Dragon (`1/5`).
//! **Rally:** Your Tavern spells give an extra `+1` (`+2` if Golden) Health this game.

use crate::cards::CardTemplate;
use crate::model::{CardId, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 307;
pub const NAME: &str = "Blue Whelp";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 5, 3)
        .with_tribe(Tribe::Dragon)
        .on_rally(|c| {
            on_rally(&c.board[c.attacker_pos], c.auras);
            Vec::new()
        })
}

pub fn on_rally(attacker: &Unit, auras: &mut PlayerAuras) {
    let delta = if attacker.is_golden { 2 } else { 1 };
    auras.spell_bonus_hp += delta;
}
