//! `Living Azerite` (`BG28_707`) — Tier 5 Elemental (`8/6`).
//!
//! Whenever you cast a Tavern spell, give Elementals in the Tavern `+4/+3` (`+8/+6` if Golden) this game.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 531;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Living Azerite", 8, 6, 5)
        .with_tribe(Tribe::Elemental)
        .on_spell_cast(on_spell_cast)
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
