//! `Deathstrider` (`BG36_208`) — Tier 6 Beast (`10/11`).
//!
//! After a friendly Rally minion attacks, trigger your left-most Deathrattle (`twice` if Golden).

use crate::cards::{self, CardTemplate, DeathrattleContext};
use crate::events::Event;
use crate::model::{CardId, PlayerAuras, Side, Tribe, Unit, UnitId};
use crate::rng::Rng;

pub const ID: CardId = 607;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Deathstrider", 10, 11, 6).with_tribe(Tribe::Beast)
}

#[allow(clippy::too_many_arguments)]
pub fn after_rally_minion_attacks(
    side: Side,
    in_combat: bool,
    board: &mut Vec<Unit>,
    auras: &mut PlayerAuras,
    hand: &mut Vec<Unit>,
    hand_summoned: &mut Vec<bool>,
    dead_aberrations: &[Unit],
    combat_beast_bonus_atk: i32,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    let mut triggers = 0u32;
    for u in board.iter() {
        if u.health > 0 && u.card_id == ID {
            triggers += if u.is_golden { 2 } else { 1 };
        }
    }
    if triggers == 0 {
        return;
    }
    let dr_mult = 1 + cards::extra_deathrattle_triggers(board);
    for _ in 0..triggers {
        let Some(dr_idx) = board
            .iter()
            .position(|u| u.health > 0 && cards::is_deathrattle_minion(u.card_id))
        else {
            break;
        };
        let dr_unit = board[dr_idx].clone();
        let mut cursor = dr_idx + 1;
        let mut dr_ctx = DeathrattleContext {
            side,
            in_combat,
            board,
            cursor: &mut cursor,
            auras,
            hand,
            hand_summoned,
            dead_aberrations,
            combat_beast_bonus_atk,
            next_id,
            rng,
            events,
        };
        for _ in 0..dr_mult {
            cards::on_deathrattle(&dr_unit, &mut dr_ctx);
        }
    }
}
