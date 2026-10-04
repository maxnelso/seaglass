//! `Sacrificial Wrathguard` (`BG36_362`) — Tier 4 Demon (`5/3`).
//!
//! Deathrattle: Give minions in the Tavern `+2/+2` (`+4/+4` if Golden) this game.
//! Activate (1): Improve this.

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 450;
pub const ACTIVATE_COST: u32 = 1;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Sacrificial Wrathguard", 5, 3, 4)
        .with_tribe(Tribe::Demon)
        .with_activate_cost(ACTIVATE_COST)
}

pub fn on_activate(state: &mut TavernState, source_pos: usize) {
    if source_pos < state.board.len() {
        let bonus = if state.board[source_pos].is_golden { 4 } else { 2 };
        state.board[source_pos].wrathguard_bonus += bonus;
    }
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let base = if dying.is_golden { 4 } else { 2 };
    let buff = base + dying.wrathguard_bonus;
    ctx.auras.tavern_all_atk += buff;
    ctx.auras.tavern_all_hp += buff;
}
