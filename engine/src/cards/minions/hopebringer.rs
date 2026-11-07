//! `Hopebringer` (`BG36_364`) — Tier 5 Dragon (`4/3`).
//!
//! Start of Combat: Give your minions `+4/+3` (`+8/+6` if Golden). (Permanently improves after a friendly minion loses Divine Shield!)

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Side, Tribe, Unit, UnitId};

pub const ID: CardId = 526;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Hopebringer", 4, 3, 5)
        .with_tribe(Tribe::Dragon)
        .on_start_of_combat(|c, id, is_golden| {
            on_start_of_combat(c.side, c.board, id, is_golden, c.events)
        })
        .on_friendly_divine_shield_lost(|unit| unit.stacks += 1)
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
        .map(|u| u.stacks)
    else {
        return;
    };
    let mult = (1 + stacks) * (if is_golden { 2 } else { 1 });
    let d_atk = 4 * mult;
    let d_hp = 3 * mult;
    for idx in 0..board.len() {
        crate::cards::buff_board_unit(side, board, idx, d_atk, d_hp, "Hopebringer", true, events);
    }
}
