//! `Sha of Fear` (`BG36_111`) — Tier 7 Aberration (`10/13`).
//!
//! Whenever you cast a Tavern spell, give your minions and Deity `+3/+3` (`+6/+6` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 708;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Sha of Fear", 10, 13, 7)
        .with_tribe(Tribe::Aberration)
        .on_spell_cast(on_spell_cast)
}

pub fn on_spell_cast(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let buff = 3 * state.board[self_idx].golden_mult();
    for u in &mut state.board {
        u.add_stats(buff, buff);
    }
    state.auras.deity.attack += buff;
    state.auras.deity.health += buff;
}
