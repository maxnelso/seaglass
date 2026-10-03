//! `Accord-o-Tron` (`BG26_147`) — Tier 3 Mech (`3/3`).
//! **Magnetic**. At the start of your turn, gain `1` (`2` if Golden) Gold.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 302;
pub const NAME: &str = "Accord-o-Tron";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 3)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::Magnetic)
}

pub fn on_start_turn(state: &mut TavernState) {
    let mut bonus = 0u32;
    for u in &state.board {
        bonus += u.sot_gold_bonus;
        if u.card_id == ID {
            bonus += if u.is_golden { 2 } else { 1 };
        }
    }
    state.gold += bonus;
}
