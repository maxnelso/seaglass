//! `Futurefin` (`BG34_145`) — Tier 7 Murloc (`7/13`).
//!
//! At the end of your turn, give this minion's stats (double if Golden) to the left-most minion in your hand.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 703;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Futurefin", 7, 13, 7).with_tribe(Tribe::Murloc)
}

pub fn on_end_turn(state: &mut TavernState) {
    let (mut total_atk, mut total_hp) = (0i32, 0i32);
    for u in &state.board {
        if u.card_id == ID {
            let mult = if u.is_golden { 2 } else { 1 };
            total_atk += u.attack.max(0) * mult;
            total_hp += u.health.max(0) * mult;
        }
    }
    if total_atk == 0 && total_hp == 0 {
        return;
    }
    if let Some(target) = state.hand.iter_mut().find(|h| !h.is_spell) {
        target.add_stats(total_atk, total_hp);
    }
}
