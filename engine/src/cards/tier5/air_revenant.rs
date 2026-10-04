//! `Air Revenant` (`BG34_858`) — Tier 5 Elemental (`3/6`).
//!
//! After you spend 7 Gold, cast `Easterly Winds` (`twice` if Golden).

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 501;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Air Revenant", 3, 6, 5)
        .with_tribe(Tribe::Elemental)
        .on_gold_spent(on_gold_spent)
}

pub fn on_gold_spent(
    state: &mut TavernState,
    self_idx: usize,
    amount: u32,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let revenant = &mut state.board[self_idx];
    revenant.counter += amount as i32;
    let casts = revenant.counter / 7 * revenant.golden_mult();
    revenant.counter %= 7;
    for _ in 0..casts {
        state.auras.spells_played += 1;
        let spell = spells::spell_by_name("Easterly Winds")
            .expect("Easterly Winds must exist in spell catalog");
        spells::cast_spell(state, spell, 0, pool, rng);
    }
}
