//! `Timecap'n Hooktail` (`BG27_005`) — Tier 3 Dragon/Pirate (`1/4`).
//! Whenever you cast a Tavern spell, give your minions `+1` Attack (`+1` Attack twice if Golden).
//!
//! It also reacts to the Tavern spells your minions cast in combat (`Runic Arcanist`, `Crimson
//! Vindicator`; `docs/combat.md` §2), before each takes effect: that Attack is for that combat only.

use crate::cards::{CardTemplate, CombatCastCtx};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 335;
pub const NAME: &str = "Timecap'n Hooktail";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 4, 3)
        .with_tribe(Tribe::DragonPirate)
        .on_spell_cast(on_spell_cast)
        .on_combat_spell_cast(on_combat_spell_cast)
}

pub fn on_spell_cast(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let bonus_atk = state.board[self_idx].golden_mult();
    for u in &mut state.board {
        u.add_stats(bonus_atk, 0);
    }
}

/// A Tavern spell cast in combat: the minions on the combat board.
pub fn on_combat_spell_cast(ctx: &mut CombatCastCtx<'_>, self_idx: usize) {
    let bonus_atk = ctx.board[self_idx].golden_mult();
    for idx in 0..ctx.board.len() {
        if ctx.board[idx].health > 0 {
            ctx.buff_unit(idx, bonus_atk, 0, NAME);
        }
    }
}
