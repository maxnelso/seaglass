//! `Wolf Pup` (`BG36_207`) — Tier 3 Beast (`3/6`).
//! **Rally:** Give your other minions `+4/+1` (`+8/+2` if Golden).
//!
//! Triggered in the Tavern (`Sky-hatch Runaway`, an attack on a `Fishbait`), it buffs your other
//! minions on the board (`docs/tavern.md` §7.7).

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Side, Tribe, Unit, UnitId};

pub const ID: CardId = 343;
pub const NAME: &str = "Wolf Pup";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 6, 3)
        .with_tribe(Tribe::Beast)
        .on_rally(|c| {
            on_rally(c.side, c.board, c.attacker_id, c.is_golden, c.events);
            Vec::new()
        })
}

pub fn on_rally(
    side: Side,
    board: &mut [Unit],
    attacker_id: UnitId,
    is_golden: bool,
    events: &mut Vec<Event>,
) {
    let m = if is_golden { 2 } else { 1 };
    let d_atk = 4 * m;
    let d_hp = m;
    let in_combat = crate::cards::in_combat_resolution();
    for idx in 0..board.len() {
        if board[idx].id != attacker_id && board[idx].health > 0 {
            crate::cards::buff_board_unit(side, board, idx, d_atk, d_hp, NAME, in_combat, events);
        }
    }
}
