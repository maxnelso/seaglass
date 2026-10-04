//! `Hackerfin` (`BG31_148`) — Tier 5 Murloc (`5/3`).
//!
//! Battlecry: Give your other minions `+3/+2` (`+6/+4` if Golden). (Improved by each different Bonus Keyword in your warband!)

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit, BONUS_KEYWORDS};
use crate::tavern::TavernState;

pub const ID: CardId = 525;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Hackerfin", 5, 3, 5).with_tribe(Tribe::Murloc)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let distinct_kws = BONUS_KEYWORDS
        .iter()
        .filter(|&&kw| state.board.iter().any(|u| u.has_keyword(kw)))
        .count() as i32;
    let mult = (1 + distinct_kws) * (if unit.is_golden { 2 } else { 1 });
    let d_atk = 3 * mult;
    let d_hp = 2 * mult;
    for u in &mut state.board {
        u.add_stats(d_atk, d_hp);
    }
}
