//! `Tichondrius` (`BG26_523`) — Tier 5 Demon (`4/4`).
//!
//! After your hero takes damage, give your Demons `+4/+4` (`+8/+8` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 550;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Tichondrius", 4, 4, 5).with_tribe(Tribe::Demon)
}

pub fn on_hero_damage_taken(board: &mut [Unit]) {
    let mut total_buff = 0i32;
    for u in board.iter() {
        if u.card_id == ID {
            total_buff += if u.is_golden { 8 } else { 4 };
        }
    }
    if total_buff > 0 {
        for u in board.iter_mut() {
            if u.tribe.matches(Tribe::Demon) {
                u.add_stats(total_buff, total_buff);
            }
        }
    }
}
