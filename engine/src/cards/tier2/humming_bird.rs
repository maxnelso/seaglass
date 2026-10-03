//! `Humming Bird` (`BG26_805`) — Tier 2 Beast (`1/4`).
//! **Start of Combat:** For the rest of this combat, your Beasts have `+1` (`+2` if Golden) Attack.

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Side, Tribe, Unit};

pub const ID: CardId = 214;
pub const NAME: &str = "Humming Bird";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 4, 2).with_tribe(Tribe::Beast)
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
    for u in board.iter_mut() {
        if u.tribe.matches(Tribe::Beast) {
            u.add_stats(delta, 0);
            events.push(Event::StatBuff {
                side,
                unit: u.id,
                atk_delta: delta,
                hp_delta: 0,
                attack: u.attack,
                health: u.health,
                reason: "StartOfCombat",
            });
        }
    }
}
