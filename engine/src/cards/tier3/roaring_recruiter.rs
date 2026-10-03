//! `Roaring Recruiter` (`BG29_816`) — Tier 3 Dragon (`2/8`).
//! Whenever another friendly Dragon attacks, give it `+3/+1` (`+6/+2` if Golden).

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Side, Tribe, Unit, UnitId};

pub const ID: CardId = 329;
pub const NAME: &str = "Roaring Recruiter";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 8, 3).with_tribe(Tribe::Dragon)
}

pub fn on_friendly_attack(
    side: Side,
    board: &mut [Unit],
    attacker_id: UnitId,
    events: &mut Vec<Event>,
) {
    let Some(atk_idx) = board.iter().position(|u| u.id == attacker_id) else {
        return;
    };
    if !board[atk_idx].tribe.matches(Tribe::Dragon) {
        return;
    }
    let mut bonus_mult = 0i32;
    for u in board.iter() {
        if u.card_id == ID && u.id != attacker_id && u.health > 0 {
            bonus_mult += if u.is_golden { 2 } else { 1 };
        }
    }
    if bonus_mult > 0 {
        let d_atk = 3 * bonus_mult;
        let d_hp = bonus_mult;
        let target = &mut board[atk_idx];
        target.add_stats(d_atk, d_hp);
        events.push(Event::StatBuff {
            side,
            unit: target.id,
            atk_delta: d_atk,
            hp_delta: d_hp,
            attack: target.attack,
            health: target.health,
            reason: NAME,
        });
    }
}
