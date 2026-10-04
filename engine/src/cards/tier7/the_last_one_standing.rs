//! `The Last One Standing` (`BG34_320`) — Tier 7 All (`15/15`).
//!
//! Rally: Give a friendly minion of each type `+15/+15` permanently (`twice` if Golden).

use crate::cards::{spells, CardTemplate};
use crate::events::Event;
use crate::model::{CardId, Side, Tribe, Unit};
use crate::rng::Rng;

pub const ID: CardId = 712;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "The Last One Standing", 15, 15, 7)
        .with_tribe(Tribe::All)
        .on_rally(|c| {
            on_rally(c.side, c.board, c.is_golden, c.in_combat, c.rng, c.events);
            Vec::new()
        })
}

pub fn on_rally(
    side: Side,
    board: &mut [Unit],
    is_golden: bool,
    in_combat: bool,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    let repeats = if is_golden { 2 } else { 1 };
    for _ in 0..repeats {
        let indices = spells::select_menagerie_targets(board, rng);
        for idx in indices {
            let target = &mut board[idx];
            target.add_stats(15, 15);
            if in_combat {
                target.perm_atk_gained += 15;
                target.perm_hp_gained += 15;
            }
            events.push(Event::StatBuff {
                side,
                unit: target.id,
                atk_delta: 15,
                hp_delta: 15,
                attack: target.attack,
                health: target.health,
                reason: "The Last One Standing",
            });
        }
    }
}
