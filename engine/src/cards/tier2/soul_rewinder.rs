//! `Soul Rewinder` (`BG26_174`) — Tier 2 Demon (`4/2`).
//! After your hero takes damage, rewind it and give this `+2` (`+4` if Golden) Health.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 228;
pub const NAME: &str = "Soul Rewinder";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 2, 2).with_tribe(Tribe::Demon)
}

/// Returns `true` if any `Soul Rewinder` on `board` rewound the hero damage.
pub fn on_hero_damage_taken(board: &mut [Unit]) -> bool {
    let mut rewound = false;
    for u in board.iter_mut() {
        if u.card_id == ID {
            let hp_buff = if u.is_golden { 4 } else { 2 };
            u.add_stats(0, hp_buff);
            rewound = true;
        }
    }
    rewound
}
