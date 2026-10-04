//! `Ultraviolet Ascendant` (`BG31_810`) — Tier 6 Elemental (`6/6`).
//!
//! Start of Combat: Give your other Elementals `+3/+3` (`+6/+6` if Golden).
//! (Improves after you play an Elemental!)

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Side, Tribe, Unit, UnitId};
use crate::tavern::TavernState;

pub const ID: CardId = 627;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Ultraviolet Ascendant", 6, 6, 6)
        .with_tribe(Tribe::Elemental)
        .on_start_of_combat(|c, id, is_golden| {
            on_start_of_combat(c.side, c.board, id, is_golden, c.events)
        })
}

pub fn after_play_minion(state: &mut TavernState, played_tribe: Tribe, board_pos: usize) {
    if !played_tribe.matches(Tribe::Elemental) {
        return;
    }
    for (idx, u) in state.board.iter_mut().enumerate() {
        if u.card_id == ID && idx != board_pos {
            u.ultraviolet_stacks += 1;
        }
    }
}

pub fn on_start_of_combat(
    side: Side,
    board: &mut [Unit],
    source_id: UnitId,
    is_golden: bool,
    events: &mut Vec<Event>,
) {
    let Some(src) = board.iter().find(|u| u.id == source_id && u.health > 0) else {
        return;
    };
    let mult = (1 + src.ultraviolet_stacks as i32) * (if is_golden { 2 } else { 1 });
    let buff = 3 * mult;
    for u in board.iter_mut() {
        if u.health > 0 && u.id != source_id && u.tribe.matches(Tribe::Elemental) {
            u.add_stats(buff, buff);
            events.push(Event::StatBuff {
                side,
                unit: u.id,
                atk_delta: buff,
                hp_delta: buff,
                attack: u.attack,
                health: u.health,
                reason: "Ultraviolet Ascendant",
            });
        }
    }
}
