//! `Ashen Corruptor` (`BG32_873`) — Tier 4 Demon (`6/6`).
//!
//! After your hero takes damage, rewind it and give minions in the Tavern `+2/+2` (`+4/+4` if Golden) this turn.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 402;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Ashen Corruptor", 6, 6, 4).with_tribe(Tribe::Demon)
}

pub fn on_hero_damage_taken(board: &[Unit], shop: &mut [Unit]) -> bool {
    let mut rewound = false;
    let mut total_buff = 0i32;
    for u in board {
        if u.card_id == ID {
            rewound = true;
            total_buff += if u.is_golden { 4 } else { 2 };
        }
    }
    if total_buff > 0 {
        for s in shop.iter_mut() {
            if !s.is_spell {
                s.add_stats(total_buff, total_buff);
            }
        }
    }
    rewound
}
