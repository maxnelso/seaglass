//! `Shamanic Tidecaller` (`BG36_704`) — Tier 5 Murloc (`5/7`).
//!
//! Whenever you cast a spell on a Murloc, give Murlocs in your hand and board `+3/+3` (`+6/+6` if Golden).
//!
//! A Murloc in the Tavern counts too (it isn't buffed itself, unlike a Murloc on your board).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

pub const ID: CardId = 546;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Shamanic Tidecaller", 5, 7, 5)
        .with_tribe(Tribe::Murloc)
        .on_after_targeted_spell(after_targeted_spell)
}

pub fn after_targeted_spell(
    state: &mut TavernState,
    self_idx: usize,
    target: MinionSlot,
    _: &mut CardPool,
    _: &mut Rng,
) {
    if !state
        .minion(target)
        .is_some_and(|u| u.tribe.matches(Tribe::Murloc))
    {
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
