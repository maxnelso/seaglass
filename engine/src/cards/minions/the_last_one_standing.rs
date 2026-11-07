//! `The Last One Standing` (`BG34_320`) — Tier 7 All (`10/10`).
//!
//! Rally: Give a friendly minion of each type `+10/+10` permanently and improve this (`twice` if
//! Golden). The minions are chosen as for `Misplaced Tea Set` (the best assignment of minions to
//! types, Naga included).

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Side, Tribe, Unit};
use crate::rng::Rng;
use crate::types;

pub const ID: CardId = 712;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "The Last One Standing", 10, 10, 7)
        .with_tribe(Tribe::All)
        .on_rally(|c| {
            on_rally(
                c.side,
                c.board,
                c.attacker_pos,
                c.is_golden,
                c.in_combat,
                c.rng,
                c.events,
            );
            Vec::new()
        })
}

pub fn on_rally(
    side: Side,
    board: &mut [Unit],
    attacker_pos: usize,
    is_golden: bool,
    in_combat: bool,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    let repeats = if is_golden { 2 } else { 1 };
    for _ in 0..repeats {
        let stacks = board.get(attacker_pos).map_or(0, |u| u.stacks);
        let buff = 10 * (1 + stacks);
        let indices = types::select_menagerie_targets(board, rng);
        for idx in indices {
            if in_combat {
                board[idx].perm_atk_gained += buff;
                board[idx].perm_hp_gained += buff;
            }
            crate::cards::buff_board_unit(
                side,
                board,
                idx,
                buff,
                buff,
                "The Last One Standing",
                in_combat,
                events,
            );
        }
        if let Some(unit) = board.get_mut(attacker_pos) {
            unit.stacks += 1;
        }
    }
}
