//! `Cage Gnawer` (`BG36_211`) — Tier 4 Beast (`2/7`).
//!
//! Whenever a friendly Beast attacks, give your Beasts `+2/+1` (`+4/+2` if Golden).

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Side, Tribe, Unit, UnitId};

pub const ID: CardId = 411;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Cage Gnawer", 2, 7, 4).with_tribe(Tribe::Beast)
}

pub fn on_friendly_attack(
    side: Side,
    board: &mut [Unit],
    attacker_id: UnitId,
    events: &mut Vec<Event>,
) {
    let Some(attacker) = board.iter().find(|u| u.id == attacker_id) else {
        return;
    };
    if !attacker.tribe.matches(Tribe::Beast) {
        return;
    }
    let mut total_atk = 0i32;
    let mut total_hp = 0i32;
    for u in board.iter() {
        if u.card_id == ID {
            let mult = if u.is_golden { 2 } else { 1 };
            total_atk += 2 * mult;
            total_hp += mult;
        }
    }
    if total_atk == 0 && total_hp == 0 {
        return;
    }
    for u in board.iter_mut() {
        if u.tribe.matches(Tribe::Beast) {
            u.add_stats(total_atk, total_hp);
            events.push(Event::StatBuff {
                side,
                unit: u.id,
                atk_delta: total_atk,
                hp_delta: total_hp,
                attack: u.attack,
                health: u.health,
                reason: "Cage Gnawer",
            });
        }
    }
}
