//! `Gatekeeper Amalgam` (`BG36_640`) — Tier 6 All (`6/6`).
//!
//! Whenever you cast a spell on this, it casts `Misplaced Tea Set` (`twice` if Golden).
//!
//! Its `Misplaced Tea Set` counts as a Tavern spell you cast (`docs/tavern.md` §7.4): it counts
//! toward `tavern_spells_cast`, triggers the spell-cast observers (`Felboar`, the "whenever you cast
//! a Tavern spell" minions) and becomes the last Tavern spell you cast (`Cataclysmic Harbinger`).
//! A Blood Gem one of your minions plays on it in the Tavern Phase counts as a spell cast on it (a
//! `Misplaced Tea Set` per Blood Gem); one a spell plays doesn't.

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

pub const ID: CardId = 612;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Gatekeeper Amalgam", 6, 6, 6)
        .with_tribe(Tribe::All)
        .on_after_targeted_spell(after_targeted_spell)
}

pub fn after_targeted_spell(
    state: &mut TavernState,
    self_idx: usize,
    target: MinionSlot,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if target != MinionSlot::Board(self_idx) {
        return;
    }
    let count = if state.board[self_idx].is_golden {
        2
    } else {
        1
    };
    for _ in 0..count {
        let tea_set = spells::spell_by_id(spells::SPELL_MISPLACED_TEA_SET)
            .expect("SPELL_MISPLACED_TEA_SET exists");
        spells::cast_spell_by_effect(state, tea_set, MinionSlot::Board(0), pool, rng);
    }
}
