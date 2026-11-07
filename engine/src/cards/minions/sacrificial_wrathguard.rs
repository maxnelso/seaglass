//! `Sacrificial Wrathguard` (`BG36_362`) — Tier 4 Demon (`6/3`).
//!
//! Deathrattle: Give minions in the Tavern `+2/+1` (`+4/+2` if Golden) this game.
//! Activate (1): Improve this.

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 450;
pub const ACTIVATE_COST: u32 = 1;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Sacrificial Wrathguard", 6, 3, 4)
        .with_tribe(Tribe::Demon)
        .with_activate_cost(ACTIVATE_COST)
        .on_deathrattle(on_deathrattle)
        .on_activate(|state, source_pos, _, _, _| on_activate(state, source_pos))
}

pub fn on_activate(state: &mut TavernState, source_pos: usize) {
    if source_pos < state.board.len() {
        let bonus = state.board[source_pos].golden_mult();
        state.board[source_pos].stacks += bonus;
    }
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let mult = dying.golden_mult() + dying.stacks;
    ctx.auras.tavern_all_atk += 2 * mult;
    ctx.auras.tavern_all_hp += mult;
}
