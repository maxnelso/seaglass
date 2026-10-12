//! `Runic Arcanist` (`BG36_245`) — Tier 4 Dragon (`2/4`).
//!
//! Start of Combat: Cast Shiny Ring twice (`4` times if Golden).
//!
//! Each Shiny Ring gets your spell power: the permanent bonuses plus the spell auras of the
//! friendly minions alive when it is cast (`docs/combat.md` §2).
//!
//! Each cast counts as you casting a Tavern spell (`docs/combat.md` §2): it counts for `Faceless
//! Converter`, and your "whenever you cast a Tavern spell" minions react before its `+1/+1`; it
//! doesn't become the last Tavern spell you cast (`Cataclysmic Harbinger`), and `Felboar` doesn't
//! count it.

use crate::cards::{self, BoardCtx, CardTemplate, CombatCastCtx};
use crate::model::{CardId, Tribe, UnitId};

pub const ID: CardId = 449;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Runic Arcanist", 2, 4, 4)
        .with_tribe(Tribe::Dragon)
        .on_start_of_combat(on_start_of_combat)
}

pub fn on_start_of_combat(ctx: &mut BoardCtx<'_>, _: UnitId, is_golden: bool) {
    let casts = if is_golden { 4 } else { 2 };
    let mut cast = ctx.cast_ctx();
    for _ in 0..casts {
        cards::cast_tavern_spell_in_combat(&mut cast, shiny_ring);
    }
}

/// `Shiny Ring`, cast in combat: give your minions +1/+1, plus your spell power.
fn shiny_ring(ctx: &mut CombatCastCtx<'_>) {
    let (atk, hp) = cards::spell_stat_buff_on_board(ctx.board, ctx.auras, 1, 1);
    for idx in 0..ctx.board.len() {
        ctx.buff_unit(idx, atk, hp, "Shiny Ring");
    }
}
