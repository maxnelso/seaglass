//! `Glambot` (`BG36_853`) — Tier 4 Mech (`4/4`).
//!
//! Whenever you cast a spell on a Mech, Magnetize a 4/4 Satellite to it (`twice` if Golden).

use crate::cards::{self, tokens, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 424;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Glambot", 4, 4, 4)
        .with_tribe(Tribe::Mech)
        .on_after_targeted_spell(after_targeted_spell)
}

pub fn after_targeted_spell(
    state: &mut TavernState,
    self_idx: usize,
    target_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if !state.board[target_pos].tribe.matches(Tribe::Mech) {
        return;
    }
    let count = state.board[self_idx].golden_mult();
    for _ in 0..count {
        let mut satellite = tokens::make_satellite(false);
        cards::magnetize(state, &mut satellite, target_pos, pool, rng);
    }
}
