//! `Sha of Fear` (`BG36_111`) — Tier 7 Aberration (`10/13`).
//!
//! Whenever you cast a Tavern spell, give your minions and Deity `+3/+3` (`+6/+6` if Golden).
//!
//! It also reacts to the Tavern spells your minions cast in combat (`Runic Arcanist`, `Crimson
//! Vindicator`; `docs/combat.md` §2), before each takes effect: the minions on the combat board (an
//! awakened Deity among them) have the stats for that combat only, while the Deity's are kept.

use crate::cards::{CardTemplate, CombatCastCtx};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 708;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Sha of Fear", 10, 13, 7)
        .with_tribe(Tribe::Aberration)
        .on_spell_cast(on_spell_cast)
        .on_combat_spell_cast(on_combat_spell_cast)
}

pub fn on_spell_cast(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let buff = 3 * state.board[self_idx].golden_mult();
    for u in &mut state.board {
        u.add_stats(buff, buff);
    }
    state.auras.deity.attack += buff;
    state.auras.deity.health += buff;
}

/// A Tavern spell cast in combat: the minions on the combat board (an awakened Deity among them),
/// then the Deity's stats.
pub fn on_combat_spell_cast(ctx: &mut CombatCastCtx<'_>, self_idx: usize) {
    let buff = 3 * ctx.board[self_idx].golden_mult();
    for idx in 0..ctx.board.len() {
        if ctx.board[idx].health > 0 {
            ctx.buff_unit(idx, buff, buff, "Sha of Fear");
        }
    }
    ctx.auras.deity.attack += buff;
    ctx.auras.deity.health += buff;
}
