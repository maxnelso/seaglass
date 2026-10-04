//! `Snazzy Phantom` (`BG36_515`) — Tier 6 Undead (`6/8`).
//!
//! After a friendly minion is Reborn, give stats equal to (`double` if Golden) its Attack to your right-most Undead.

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Side, Tribe, Unit};

pub const ID: CardId = 622;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Snazzy Phantom", 6, 8, 6).with_tribe(Tribe::Undead)
}

pub fn on_friendly_reborn(
    side: Side,
    board: &mut [Unit],
    reborn_attack: i32,
    events: &mut Vec<Event>,
) {
    let atk_val = reborn_attack.max(0);
    if atk_val == 0 {
        return;
    }
    let mut total_mult = 0i32;
    for u in board.iter() {
        if u.health > 0 && u.card_id == ID {
            total_mult += if u.is_golden { 2 } else { 1 };
        }
    }
    if total_mult == 0 {
        return;
    }
    let buff = atk_val * total_mult;
    if let Some(target_idx) = board
        .iter()
        .rposition(|u| u.health > 0 && u.tribe.matches(Tribe::Undead))
    {
        let target = &mut board[target_idx];
        target.add_stats(buff, buff);
        events.push(Event::StatBuff {
            side,
            unit: target.id,
            atk_delta: buff,
            hp_delta: buff,
            attack: target.attack,
            health: target.health,
            reason: "Snazzy Phantom",
        });
    }
}
