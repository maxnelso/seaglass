//! `Gatekeeper Amalgam` (`BG36_640`) — Tier 6 All (`6/6`).
//!
//! Whenever you cast a spell on this, it casts `Misplaced Tea Set` (`twice` if Golden).

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 612;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Gatekeeper Amalgam", 6, 6, 6)
        .with_tribe(Tribe::All)
        .on_after_targeted_spell(after_targeted_spell)
}

pub fn after_targeted_spell(
    state: &mut TavernState,
    self_idx: usize,
    target_pos: usize,
    _: &mut CardPool,
    rng: &mut Rng,
) {
    if self_idx != target_pos {
        return;
    }
    let count = if state.board[self_idx].is_golden {
        2
    } else {
        1
    };
    for _ in 0..count {
        spells::apply_misplaced_tea_set(state, rng);
    }
}
