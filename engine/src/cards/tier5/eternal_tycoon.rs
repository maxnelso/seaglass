//! `Eternal Tycoon` (`BG34_403`) — Tier 5 Undead (`4/8`).
//!
//! Avenge (5): Summon an `Eternal Knight` (or a Golden `Eternal Knight` if Golden). It attacks immediately.

use crate::cards::{apply_combat_summon_modifiers, tier2, CardTemplate};
use crate::combat::MAX_BOARD_SIZE;
use crate::events::Event;
use crate::model::{CardId, PlayerAuras, Side, Tribe, Unit, UnitId};

pub const ID: CardId = 517;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Eternal Tycoon", 4, 8, 5).with_tribe(Tribe::Undead)
}

#[allow(clippy::too_many_arguments)]
pub fn on_friendly_death(
    side: Side,
    surviving_board: &mut Vec<Unit>,
    auras: &PlayerAuras,
    combat_beast_bonus_atk: i32,
    next_id: &mut UnitId,
    events: &mut Vec<Event>,
    immediate_attackers: &mut Vec<UnitId>,
) {
    let mut spawns: Vec<(UnitId, bool)> = Vec::new();
    for u in surviving_board.iter_mut() {
        if u.card_id == ID && u.health > 0 {
            u.avenge_counter += 1;
            if u.avenge_counter >= 5 {
                u.avenge_counter -= 5;
                spawns.push((u.id, u.is_golden));
            }
        }
    }
    for (src_id, is_golden) in spawns {
        if surviving_board.len() >= MAX_BOARD_SIZE {
            break;
        }
        let Some(pos) = surviving_board.iter().position(|u| u.id == src_id) else {
            continue;
        };
        let mut knight = tier2::eternal_knight::template().instantiate();
        if is_golden {
            knight.make_golden();
        }
        knight.id = *next_id;
        *next_id += 1;
        apply_combat_summon_modifiers(
            surviving_board,
            auras,
            combat_beast_bonus_atk,
            knight.id,
            &mut knight,
        );
        let kid = knight.id;
        events.push(Event::UnitSummoned {
            side,
            source: src_id,
            unit: kid,
            name: knight.name.clone(),
            attack: knight.attack,
            health: knight.health,
            reason: "Eternal Tycoon",
        });
        surviving_board.insert(pos + 1, knight);
        immediate_attackers.push(kid);
    }
}
