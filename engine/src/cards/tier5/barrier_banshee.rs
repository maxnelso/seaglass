//! `Barrier Banshee` (`BG36_514`) — Tier 5 Undead (`8/8`).
//!
//! After a friendly minion is Reborn, gain Divine Shield and `+8/+8` (`+16/+16` if Golden).

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Keyword, Side, Tribe, Unit};

pub const ID: CardId = 502;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Barrier Banshee", 8, 8, 5).with_tribe(Tribe::Undead)
}

pub fn on_friendly_reborn(side: Side, board: &mut [Unit], events: &mut Vec<Event>) {
    for u in board.iter_mut() {
        if u.card_id == ID && u.health > 0 {
            let buff = if u.is_golden { 16 } else { 8 };
            u.apply_keyword(Keyword::DivineShield, false);
            u.add_stats(buff, buff);
            events.push(Event::StatBuff {
                side,
                unit: u.id,
                atk_delta: buff,
                hp_delta: buff,
                attack: u.attack,
                health: u.health,
                reason: "Barrier Banshee",
            });
        }
    }
}
