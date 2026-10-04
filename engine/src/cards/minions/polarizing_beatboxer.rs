//! `Polarizing Beatboxer` (`BG26_149`) — Tier 7 Mech (`5/10`).
//!
//! Whenever you Magnetize to a different minion, it also Magnetizes to this (`twice` if Golden).

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 707;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Polarizing Beatboxer", 5, 10, 7)
        .with_tribe(Tribe::Mech)
        .on_after_friendly_magnetize(after_friendly_magnetize)
}

pub fn after_friendly_magnetize(
    state: &mut TavernState,
    self_idx: usize,
    card: &Unit,
    target_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if self_idx == target_pos {
        return;
    }
    let base = if state.board[self_idx].is_golden {
        2
    } else {
        1
    };
    let extra = 1 + cards::extra_magnetizations(&mut state.board[self_idx]);
    for _ in 0..base * extra {
        cards::apply_magnetization(state, card, self_idx, pool, rng);
    }
}
