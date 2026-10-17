//! `Vicious Mindslasher` (`BG36_108`) — Tier 3 Aberration (`1/2`).
//! Whenever you cast a Tavern spell, give this and your **Deity** `+1/+2` (`+2/+4` if Golden).
//!
//! It also reacts to the Tavern spells your minions cast in combat (`Runic Arcanist`, `Crimson
//! Vindicator`; `docs/combat.md` §2), before each takes effect: its own stats are for that combat
//! only, while the Deity's are kept (an awakened Deity also gets them on the board).
//!
//! With `Mindslasher Portrait` (`trinkets::modifies`, `docs/trinkets.md` §11.8), its adjacent
//! minions gain the same stats (for that combat only in combat).

use crate::cards::{CardTemplate, CombatCastCtx};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 340;
pub const NAME: &str = "Vicious Mindslasher";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 2, 3)
        .with_tribe(Tribe::Aberration)
        .on_spell_cast(on_spell_cast)
        .on_combat_spell_cast(on_combat_spell_cast)
}

pub fn on_spell_cast(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let m = state.board[self_idx].golden_mult();
    state.board[self_idx].add_stats(m, 2 * m);
    state.auras.deity.attack += m;
    state.auras.deity.health += 2 * m;
    if crate::trinkets::modifies(&state.auras, ID) {
        for idx in [self_idx.wrapping_sub(1), self_idx + 1] {
            if let Some(unit) = state.board.get_mut(idx) {
                unit.add_stats(m, 2 * m);
            }
        }
    }
}

/// A Tavern spell cast in combat: this, the Deity's stats, and the Deity on the board if it has
/// awakened.
pub fn on_combat_spell_cast(ctx: &mut CombatCastCtx<'_>, self_idx: usize) {
    let m = ctx.board[self_idx].golden_mult();
    ctx.buff_unit(self_idx, m, 2 * m, NAME);
    ctx.auras.deity.attack += m;
    ctx.auras.deity.health += 2 * m;
    for idx in 0..ctx.board.len() {
        if ctx.board[idx].is_deity && ctx.board[idx].health > 0 {
            ctx.buff_unit(idx, m, 2 * m, NAME);
        }
    }
    if crate::trinkets::modifies(ctx.auras, ID) {
        for idx in [self_idx.wrapping_sub(1), self_idx + 1] {
            if ctx.board.get(idx).is_some_and(|u| u.health > 0) {
                ctx.buff_unit(idx, m, 2 * m, NAME);
            }
        }
    }
}
