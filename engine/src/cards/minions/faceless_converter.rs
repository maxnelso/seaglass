//! `Faceless Converter` (`BG36_318`) — Tier 5 Aberration (`5/5`).
//!
//! Deathrattle: Give your Deity `+2/+1` (`+4/+2` if Golden). (Improved by each Tavern spell you've cast this game!)

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 518;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Faceless Converter", 5, 5, 5)
        .with_tribe(Tribe::Aberration)
        .on_deathrattle(on_deathrattle)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let mult = (1 + ctx.auras.spells_played as i32) * (if dying.is_golden { 2 } else { 1 });
    let d_atk = 2 * mult;
    let d_hp = mult;
    ctx.auras.deity.attack += d_atk;
    ctx.auras.deity.health += d_hp;
    for idx in 0..ctx.board.len() {
        if ctx.board[idx].is_deity {
            ctx.buff_unit(idx, d_atk, d_hp, "Faceless Converter");
        }
    }
}
