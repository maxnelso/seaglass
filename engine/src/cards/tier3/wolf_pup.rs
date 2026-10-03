//! `Wolf Pup` (`BG36_207`) — Tier 3 Beast (`3/6`).
//! **Rally:** Give your other minions `+4/+1` (`+8/+2` if Golden).

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Side, Tribe, Unit, UnitId};

pub const ID: CardId = 343;
pub const NAME: &str = "Wolf Pup";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 6, 3).with_tribe(Tribe::Beast)
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
    for u in board.iter_mut() {
        if u.id != attacker_id && u.health > 0 {
            u.add_stats(d_atk, d_hp);
            events.push(Event::StatBuff {
                side,
                unit: u.id,
                atk_delta: d_atk,
                hp_delta: d_hp,
                attack: u.attack,
                health: u.health,
                reason: NAME,
            });
        }
    }
}
