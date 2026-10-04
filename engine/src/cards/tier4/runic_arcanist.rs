//! `Runic Arcanist` (`BG36_245`) — Tier 4 Dragon (`2/4`).
//!
//! Start of Combat: Cast Shiny Ring twice (`4` times if Golden).

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, PlayerAuras, Side, Tribe, Unit};

pub const ID: CardId = 449;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Runic Arcanist", 2, 4, 4)
        .with_tribe(Tribe::Dragon)
        .on_start_of_combat(|c, _, is_golden| {
            on_start_of_combat(c.side, c.board, c.auras, is_golden, c.events)
        })
}

pub fn on_start_of_combat(
    side: Side,
    board: &mut [Unit],
    auras: &PlayerAuras,
    is_golden: bool,
    events: &mut Vec<Event>,
) {
    let casts = if is_golden { 4 } else { 2 };
    let (atk, hp) = auras.spell_stat_buff(1, 1);
    for _ in 0..casts {
        for u in board.iter_mut() {
            u.add_stats(atk, hp);
            events.push(Event::StatBuff {
                side,
                unit: u.id,
                atk_delta: atk,
                hp_delta: hp,
                attack: u.attack,
                health: u.health,
                reason: "Shiny Ring",
            });
        }
    }
}
