//! `Living Azerite` (`BG28_707`) — Tier 5 Elemental (`8/6`).
//!
//! Whenever you cast a Tavern spell, give Elementals in the Tavern `+4/+3` (`+8/+6` if Golden) this game.
//!
//! It also reacts to the Tavern spells your minions cast in combat (`Runic Arcanist`, `Crimson
//! Vindicator`; `docs/combat.md` §2): the bonus is kept, and the Elementals in the Tavern get it once
//! the combat is over.

use crate::cards::{CardTemplate, CombatCastCtx};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 531;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Living Azerite", 8, 6, 5)
        .with_tribe(Tribe::Elemental)
        .on_spell_cast(on_spell_cast)
        .on_combat_spell_cast(on_combat_spell_cast)
}

pub fn on_spell_cast(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let mult = state.board[self_idx].golden_mult();
    let (d_atk, d_hp) = (4 * mult, 3 * mult);
    state.auras.tavern_elemental_atk += d_atk;
    state.auras.tavern_elemental_hp += d_hp;
    for s in &mut state.shop {
        if !s.is_spell && s.tribe.matches(Tribe::Elemental) {
            s.add_stats(d_atk, d_hp);
        }
    }
}

/// A Tavern spell cast in combat: the bonus only (the Elementals in the Tavern get it once the
/// combat is over).
pub fn on_combat_spell_cast(ctx: &mut CombatCastCtx<'_>, self_idx: usize) {
    let mult = ctx.board[self_idx].golden_mult();
    ctx.auras.tavern_elemental_atk += 4 * mult;
    ctx.auras.tavern_elemental_hp += 3 * mult;
}
