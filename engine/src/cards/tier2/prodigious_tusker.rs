//! `Prodigious Tusker` (`BG33_430`) — Tier 2 Quilboar (`2/5`).
//! Whenever another friendly minion attacks, this plays a (`2` if Golden) **Blood Gem(s)** on it.

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, PlayerAuras, Side, Tribe, Unit, UnitId};

pub const ID: CardId = 222;
pub const NAME: &str = "Prodigious Tusker";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 5, 2).with_tribe(Tribe::Quilboar)
}

pub fn on_friendly_attack(
    side: Side,
    board: &mut [Unit],
    attacker_id: UnitId,
    auras: &PlayerAuras,
    events: &mut Vec<Event>,
) {
    let total_gems: u32 = board
        .iter()
        .filter(|u| u.id != attacker_id && u.card_id == ID)
        .map(|u| if u.is_golden { 2 } else { 1 })
        .sum();

    if total_gems == 0 {
        return;
    }

    if let Some(attacker) = board.iter_mut().find(|u| u.id == attacker_id) {
        let pre_atk = attacker.attack;
        let pre_hp = attacker.health;
        attacker.play_blood_gems(total_gems, auras);
        let post_atk = attacker.attack;
        let post_hp = attacker.health;
        if post_atk != pre_atk || post_hp != pre_hp {
            events.push(Event::StatBuff {
                side,
                unit: attacker_id,
                atk_delta: post_atk - pre_atk,
                hp_delta: post_hp - pre_hp,
                attack: post_atk,
                health: post_hp,
                reason: "ProdigiousTusker",
            });
        }
    }
}
