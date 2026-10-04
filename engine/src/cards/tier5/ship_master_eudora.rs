//! `Ship Master Eudora` (`BG33_828`) — Tier 5 Pirate (`8/3`).
//!
//! Deathrattle: Give your minions `+6/+6` (`twice` if Golden). Golden ones keep it permanently.

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 547;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Ship Master Eudora", 8, 3, 5).with_tribe(Tribe::Pirate)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let amount = if dying.is_golden { 12 } else { 6 };
    for idx in 0..ctx.board.len() {
        ctx.buff_unit(idx, amount, amount, "Ship Master Eudora");
        if ctx.in_combat && ctx.board[idx].is_golden {
            ctx.board[idx].perm_atk_gained += amount;
            ctx.board[idx].perm_hp_gained += amount;
        }
    }
}
