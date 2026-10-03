//! `Electric Synthesizer` (`BG26_963`) — Tier 2 Dragon (`3/4`).
//! **Battlecry and Start of Combat:** Give your other Dragons `+1/+1` (`+2/+2` if Golden).

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Side, Tribe, Unit, UnitId};
use crate::tavern::TavernState;

pub const ID: CardId = 208;
pub const NAME: &str = "Electric Synthesizer";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 4, 2).with_tribe(Tribe::Dragon)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let delta = if unit.is_golden { 2 } else { 1 };
    for other in &mut state.board {
        if other.tribe.matches(Tribe::Dragon) {
            other.add_stats(delta, delta);
        }
    }
}

pub fn on_start_of_combat(
    side: Side,
    board: &mut [Unit],
    source_id: UnitId,
    is_golden: bool,
    events: &mut Vec<Event>,
) {
    let delta = if is_golden { 2 } else { 1 };
    for u in board.iter_mut() {
        if u.id != source_id && u.tribe.matches(Tribe::Dragon) {
            u.add_stats(delta, delta);
            events.push(Event::StatBuff {
                side,
                unit: u.id,
                atk_delta: delta,
                hp_delta: delta,
                attack: u.attack,
                health: u.health,
                reason: "StartOfCombat",
            });
        }
    }
}
