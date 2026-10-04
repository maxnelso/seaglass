//! `Vicious Mindslasher` (`BG36_108`) — Tier 3 Aberration (`1/2`).
//! Whenever you cast a Tavern spell, give this and your **Deity** `+1/+2` (`+2/+4` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 340;
pub const NAME: &str = "Vicious Mindslasher";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 2, 3)
        .with_tribe(Tribe::Aberration)
        .on_spell_cast(on_spell_cast)
}

pub fn on_spell_cast(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let m = state.board[self_idx].golden_mult();
    state.board[self_idx].add_stats(m, 2 * m);
    state.auras.deity.attack += m;
    state.auras.deity.health += 2 * m;
}
