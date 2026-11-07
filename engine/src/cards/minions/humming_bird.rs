//! `Humming Bird` (`BG26_805`) — Tier 2 Beast (`1/4`).
//! **Start of Combat:** For the rest of this combat, your Beasts have `+1` (`+2` if Golden) Attack.

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Side, Tribe, Unit};

pub const ID: CardId = 214;
pub const NAME: &str = "Humming Bird";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 4, 2)
        .with_tribe(Tribe::Beast)
        .on_start_of_combat(|c, _, is_golden| {
            on_start_of_combat(c.side, c.board, c.beast_bonus_atk, is_golden, c.events)
        })
}

pub fn on_start_of_combat(
    side: Side,
    board: &mut [Unit],
    combat_beast_bonus_atk: &mut i32,
    is_golden: bool,
    events: &mut Vec<Event>,
) {
    let delta = if is_golden { 2 } else { 1 };
    *combat_beast_bonus_atk += delta;
    for idx in 0..board.len() {
        if board[idx].tribe.matches(Tribe::Beast) {
            crate::cards::buff_board_unit(
                side,
                board,
                idx,
                delta,
                0,
                "StartOfCombat",
                true,
                events,
            );
        }
    }
}
