//! `Diremuck Forager` (`BG27_556`) — Tier 3 Murloc (`4/5`).
//! **Start of Combat:** When you have space, summon the highest-Attack (`two highest-Attack` if Golden) Murloc(s) from your hand for this combat only.

use crate::cards::{self, CardTemplate};
use crate::combat::MAX_BOARD_SIZE;
use crate::events::Event;
use crate::model::{CardId, PlayerAuras, Side, Tribe, Unit, UnitId};

pub const ID: CardId = 311;
pub const NAME: &str = "Diremuck Forager";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 5, 3)
        .with_tribe(Tribe::Murloc)
        .on_start_of_combat(|c, id, is_golden| {
            on_start_of_combat(
                c.side,
                c.board,
                id,
                is_golden,
                c.auras,
                c.hand,
                c.hand_summoned,
                *c.beast_bonus_atk,
                c.next_id,
                c.events,
            )
        })
}

#[allow(clippy::too_many_arguments)]
pub fn on_start_of_combat(
    side: Side,
    board: &mut Vec<Unit>,
    source_id: UnitId,
    is_golden: bool,
    auras: &PlayerAuras,
    hand: &[Unit],
    hand_summoned: &mut [bool],
    combat_beast_bonus_atk: i32,
    next_id: &mut UnitId,
    events: &mut Vec<Event>,
) {
    let count = if is_golden { 2 } else { 1 };
    let Some(source_pos) = board.iter().position(|u| u.id == source_id) else {
        return;
    };
    let mut insert_pos = source_pos + 1;
    for _ in 0..count {
        if board.len() >= MAX_BOARD_SIZE {
            break;
        }
        let best = hand
            .iter()
            .enumerate()
            .filter(|(i, u)| {
                !hand_summoned.get(*i).copied().unwrap_or(true)
                    && !u.is_spell
                    && u.tribe.matches(Tribe::Murloc)
            })
            .max_by_key(|(_, u)| u.attack);
        if let Some((idx, murloc)) = best {
            hand_summoned[idx] = true;
            let mut copy = murloc.clone();
            copy.id = *next_id;
            *next_id += 1;
            cards::apply_combat_summon_modifiers(board, auras, combat_beast_bonus_atk, &mut copy);
            events.push(Event::UnitSummoned {
                side,
                source: source_id,
                unit: copy.id,
                name: copy.name.clone(),
                attack: copy.attack,
                health: copy.health,
                reason: NAME,
            });
            board.insert(insert_pos, copy);
            insert_pos += 1;
        }
    }
}
