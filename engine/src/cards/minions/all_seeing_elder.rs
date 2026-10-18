//! `All-Seeing Elder` (`BG33_300`) — Tier 4 (`1/1`), in no pool.
//!
//! **Start of Combat:** Gain the Attack of your left-most minion and the Health of your right-most
//! minion (`twice` if Golden).
//!
//! The Tier 4 Darkmoon Prize minion (`docs/darkmoon_prizes.md` §6): only a prize Discover gives it.
//! The left-most and right-most living friendly minions, itself if it is there; Golden, it gains
//! them twice, each time as they are then (engine decision).

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Side, Unit, UnitId};

pub const ID: CardId = 122104;
pub const NAME: &str = "All-Seeing Elder";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 1, 4).on_start_of_combat(|c, id, is_golden| {
        on_start_of_combat(c.side, c.board, id, is_golden, c.events)
    })
}

pub fn on_start_of_combat(
    side: Side,
    board: &mut [Unit],
    source_id: UnitId,
    is_golden: bool,
    events: &mut Vec<Event>,
) {
    for _ in 0..if is_golden { 2 } else { 1 } {
        let mut living = board.iter().filter(|u| u.health > 0);
        let Some(attack) = living.next().map(|u| u.attack) else {
            return;
        };
        let health = board
            .iter()
            .rev()
            .find(|u| u.health > 0)
            .map_or(0, |u| u.health);
        let Some(elder) = board.iter_mut().find(|u| u.id == source_id && u.health > 0) else {
            return;
        };
        elder.add_stats(attack.max(0), health.max(0));
        events.push(Event::StatBuff {
            side,
            unit: elder.id,
            atk_delta: attack.max(0),
            hp_delta: health.max(0),
            attack: elder.attack,
            health: elder.health,
            reason: NAME,
        });
    }
}
