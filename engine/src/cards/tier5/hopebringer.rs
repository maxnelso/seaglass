//! `Hopebringer` (`BG36_364`) — Tier 5 Dragon (`4/3`).
//!
//! Start of Combat: Give your minions `+4/+3` (`+8/+6` if Golden). (Permanently improves after a friendly minion loses Divine Shield!)

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Side, Tribe, Unit, UnitId};

pub const ID: CardId = 526;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Hopebringer", 4, 3, 5).with_tribe(Tribe::Dragon)
}

pub fn on_start_of_combat(
    side: Side,
    board: &mut [Unit],
    source_id: UnitId,
    is_golden: bool,
    events: &mut Vec<Event>,
) {
    let Some(stacks) = board
        .iter()
        .find(|u| u.id == source_id && u.health > 0)
        .map(|u| u.hopebringer_stacks)
    else {
        return;
    };
    let mult = (1 + stacks as i32) * (if is_golden { 2 } else { 1 });
    let d_atk = 4 * mult;
    let d_hp = 3 * mult;
    for u in board.iter_mut() {
        u.add_stats(d_atk, d_hp);
        events.push(Event::StatBuff {
            side,
            unit: u.id,
            atk_delta: d_atk,
            hp_delta: d_hp,
            attack: u.attack,
            health: u.health,
            reason: "Hopebringer",
        });
    }
}

pub fn on_friendly_divine_shield_lost(board: &mut [Unit]) {
    for u in board.iter_mut() {
        if u.card_id == ID && u.health > 0 {
            u.hopebringer_stacks += 1;
        }
    }
}
