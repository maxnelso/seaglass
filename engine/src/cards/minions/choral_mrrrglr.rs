//! `Choral Mrrrglr` (`BG26_354`) — Tier 6 Murloc (`6/6`).
//!
//! Start of Combat: Gain the stats of all the minions in your hand (`twice` if Golden).

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Side, Tribe, Unit, UnitId};

pub const ID: CardId = 603;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Choral Mrrrglr", 6, 6, 6)
        .with_tribe(Tribe::Murloc)
        .on_start_of_combat(|c, id, is_golden| {
            on_start_of_combat(c.side, c.board, id, is_golden, c.hand, c.events)
        })
}

pub fn on_start_of_combat(
    side: Side,
    board: &mut [Unit],
    source_id: UnitId,
    is_golden: bool,
    hand: &[Unit],
    events: &mut Vec<Event>,
) {
    let mult = if is_golden { 2 } else { 1 };
    let mut total_atk = 0i32;
    let mut total_hp = 0i32;
    for h in hand {
        if !h.is_spell {
            total_atk += h.attack.max(0);
            total_hp += h.health.max(0);
        }
    }
    let d_atk = total_atk * mult;
    let d_hp = total_hp * mult;
    if d_atk <= 0 && d_hp <= 0 {
        return;
    }
    if let Some(u) = board.iter_mut().find(|u| u.id == source_id && u.health > 0) {
        u.add_stats(d_atk, d_hp);
        events.push(Event::StatBuff {
            side,
            unit: u.id,
            atk_delta: d_atk,
            hp_delta: d_hp,
            attack: u.attack,
            health: u.health,
            reason: "Choral Mrrrglr",
        });
    }
}
