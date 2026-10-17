//! `Charging Czarina` (`BG28_741`) — Tier 5 Mech (`4/1`, Divine Shield).
//!
//! Divine Shield. Whenever you cast a Tavern spell, give your minions with Divine Shield `+4` (`+8` if Golden) Attack.
//!
//! It also reacts to the Tavern spells your minions cast in combat (`Runic Arcanist`, `Crimson
//! Vindicator`; `docs/combat.md` §2), before each takes effect: that Attack is for that combat only.
//!
//! With `Czarina Portrait` (`trinkets::modifies`, `docs/trinkets.md` §11.8), as much Health too.

use crate::cards::{CardTemplate, CombatCastCtx};
use crate::model::{CardId, Keyword, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 506;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Charging Czarina", 4, 1, 5)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::DivineShield)
        .on_spell_cast(on_spell_cast)
        .on_combat_spell_cast(on_combat_spell_cast)
}

pub fn on_spell_cast(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let atk = 4 * state.board[self_idx].golden_mult();
    let hp = health(&state.auras, atk);
    for u in state.board.iter_mut().filter(|u| u.divine_shield) {
        u.add_stats(atk, hp);
    }
}

/// A Tavern spell cast in combat: the minions on the combat board that have Divine Shield then.
pub fn on_combat_spell_cast(ctx: &mut CombatCastCtx<'_>, self_idx: usize) {
    let atk = 4 * ctx.board[self_idx].golden_mult();
    let hp = health(ctx.auras, atk);
    for idx in 0..ctx.board.len() {
        if ctx.board[idx].divine_shield && ctx.board[idx].health > 0 {
            ctx.buff_unit(idx, atk, hp, "Charging Czarina");
        }
    }
}

/// The Health it gives with its Attack: as much with `Czarina Portrait`, else none.
fn health(auras: &crate::model::PlayerAuras, atk: i32) -> i32 {
    if crate::trinkets::modifies(auras, ID) {
        atk
    } else {
        0
    }
}
