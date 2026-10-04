//! `Deathstrider` (`BG36_208`) — Tier 6 Beast (`10/11`).
//!
//! After a friendly Rally minion attacks, trigger your left-most Deathrattle (`twice` if Golden).

use crate::cards::{self, BoardCtx, CardTemplate, Passive};
use crate::model::{CardId, Tribe};

pub const ID: CardId = 607;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Deathstrider", 10, 11, 6).with_tribe(Tribe::Beast)
}

pub fn after_rally_minion_attacks(ctx: &mut BoardCtx<'_>) {
    let mut triggers = 0u32;
    for u in ctx.board.iter() {
        if u.health > 0 && u.card_id == ID {
            triggers += if u.is_golden { 2 } else { 1 };
        }
    }
    if triggers == 0 {
        return;
    }
    let dr_mult = 1 + cards::board_passive(ctx.board, Passive::ExtraDeathrattles);
    for _ in 0..triggers {
        let Some(dr_idx) = ctx
            .board
            .iter()
            .position(|u| u.health > 0 && cards::is_deathrattle_minion(u.card_id))
        else {
            break;
        };
        let dr_unit = ctx.board[dr_idx].clone();
        ctx.cursor = dr_idx + 1;
        for _ in 0..dr_mult {
            cards::on_deathrattle(&dr_unit, ctx);
        }
    }
}
