//! `Shamanic Tidecaller` (`BG36_704`) — Tier 5 Murloc (`5/7`).
//!
//! Whenever you cast a spell on a Murloc, give Murlocs in your hand and board `+3/+3` (`+6/+6` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 546;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Shamanic Tidecaller", 5, 7, 5)
        .with_tribe(Tribe::Murloc)
        .on_after_targeted_spell(after_targeted_spell)
}

pub fn after_targeted_spell(
    state: &mut TavernState,
    self_idx: usize,
    target_pos: usize,
    _: &mut CardPool,
    _: &mut Rng,
) {
    if !state.board[target_pos].tribe.matches(Tribe::Murloc) {
        return;
    }
    let buff = if state.board[self_idx].is_golden {
        6
    } else {
        3
    };
    for u in &mut state.board {
        if u.tribe.matches(Tribe::Murloc) {
            u.add_stats(buff, buff);
        }
    }
    for h in &mut state.hand {
        if !h.is_spell && h.tribe.matches(Tribe::Murloc) {
            h.add_stats(buff, buff);
        }
    }
}
