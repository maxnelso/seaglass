//! `Devout Hellcaller` (`BG33_155`) — Tier 3 Demon (`4/4`).
//! After another friendly Demon deals damage, gain `+2/+2` (`+4/+4` if Golden) permanently.

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Side, Tribe, Unit, UnitId};

pub const ID: CardId = 310;
pub const NAME: &str = "Devout Hellcaller";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 4, 3).with_tribe(Tribe::Demon)
}

pub fn on_friendly_dealt_damage(
    side: Side,
    board: &mut [Unit],
    source_id: UnitId,
    source_tribe: Tribe,
    events: &mut Vec<Event>,
) {
    if !source_tribe.matches(Tribe::Demon) {
        return;
    }
    for u in board.iter_mut() {
        if u.card_id == ID && u.id != source_id && u.health > 0 {
            let delta = if u.is_golden { 4 } else { 2 };
            u.add_stats(delta, delta);
            u.perm_atk_gained += delta;
            u.perm_hp_gained += delta;
            events.push(Event::StatBuff {
                side,
                unit: u.id,
                atk_delta: delta,
                hp_delta: delta,
                attack: u.attack,
                health: u.health,
                reason: NAME,
            });
        }
    }
}
