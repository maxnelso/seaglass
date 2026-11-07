//! `Champion of Sargeras` (`BG27_016`) — Tier 7 Demon (`12/12`).
//!
//! Battlecry and Deathrattle: Give minions in the Tavern `+12/+12` (`+24/+24` if Golden) this game.

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 702;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Champion of Sargeras", 12, 12, 7)
        .with_tribe(Tribe::Demon)
        .on_battlecry(|state, unit, _, _, _| on_battlecry(state, unit))
        .on_deathrattle(|dying, ctx| on_deathrattle(ctx, dying.is_golden))
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let buff = if unit.is_golden { 24 } else { 12 };
    state.auras.tavern_all_atk += buff;
    state.auras.tavern_all_hp += buff;
    for u in &mut state.shop {
        if !u.is_spell {
            u.add_stats(buff, buff);
        }
    }
}

pub fn on_deathrattle(ctx: &mut DeathrattleContext<'_>, is_golden: bool) {
    let buff = if is_golden { 24 } else { 12 };
    ctx.auras.tavern_all_atk += buff;
    ctx.auras.tavern_all_hp += buff;
}
