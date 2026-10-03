//! `Lullabot` (`BG26_146`) — Tier 1 Mech (`2/2`).
//! **Magnetic**. At the end of your turn, gain `+1` (`+2` if Golden) Health.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 111;
pub const NAME: &str = "Lullabot";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 2, 1)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::Magnetic)
}

pub fn on_end_turn(state: &mut TavernState) {
    for unit in &mut state.board {
        let mut bonus = unit.eot_health_bonus;
        if unit.card_id == ID {
            bonus += if unit.is_golden { 2 } else { 1 };
        }
        if bonus > 0 {
            unit.add_stats(0, bonus);
        }
    }
}
