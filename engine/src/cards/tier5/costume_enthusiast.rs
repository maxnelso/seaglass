//! `Costume Enthusiast` (`BG34_142`) — Tier 5 Murloc (`4/5`, Divine Shield).
//!
//! Divine Shield. Start of Combat: Gain (`double` if Golden) the Attack of the highest-Attack minion in your hand.

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Keyword, Side, Tribe, Unit, UnitId};

pub const ID: CardId = 507;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Costume Enthusiast", 4, 5, 5)
        .with_tribe(Tribe::Murloc)
        .with_keyword(Keyword::DivineShield)
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
    let max_hand_atk = hand
        .iter()
        .filter(|h| !h.is_spell)
        .map(|h| h.attack)
        .max()
        .unwrap_or(0);
    if max_hand_atk <= 0 {
        return;
    }
    let mult = if is_golden { 2 } else { 1 };
    let gain = max_hand_atk * mult;
    if let Some(u) = board.iter_mut().find(|u| u.id == source_id && u.health > 0) {
        u.add_stats(gain, 0);
        events.push(Event::StatBuff {
            side,
            unit: u.id,
            atk_delta: gain,
            hp_delta: 0,
            attack: u.attack,
            health: u.health,
            reason: "Costume Enthusiast",
        });
    }
}
