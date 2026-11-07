//! `Ultraviolet Ascendant` (`BG31_810`) — Tier 6 Elemental (`8/8`).
//!
//! Start of Combat: Give your other Elementals `+4/+4` (`+8/+8` if Golden).
//! (Improves after you play an Elemental!)

use crate::cards::{CardTemplate, Played};
use crate::events::Event;
use crate::model::{CardId, Side, Tribe, Unit, UnitId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 627;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Ultraviolet Ascendant", 8, 8, 6)
        .with_tribe(Tribe::Elemental)
        .on_start_of_combat(|c, id, is_golden| {
            on_start_of_combat(c.side, c.board, id, is_golden, c.events)
        })
        .on_after_friendly_play(after_friendly_play)
}

pub fn after_friendly_play(
    state: &mut TavernState,
    self_idx: usize,
    played: &Played,
    _: &mut CardPool,
    _: &mut Rng,
) {
    if !played.magnetized && played.tribe.matches(Tribe::Elemental) {
        state.board[self_idx].stacks += 1;
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
    let mult = (1 + src.stacks) * (if is_golden { 2 } else { 1 });
    let buff = 4 * mult;
    for idx in 0..board.len() {
        if board[idx].health > 0
            && board[idx].id != source_id
            && board[idx].tribe.matches(Tribe::Elemental)
        {
            crate::cards::buff_board_unit(
                side,
                board,
                idx,
                buff,
                buff,
                "Ultraviolet Ascendant",
                true,
                events,
            );
        }
    }
}
