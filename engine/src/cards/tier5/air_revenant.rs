//! `Air Revenant` (`BG34_858`) — Tier 5 Elemental (`3/6`).
//!
//! After you spend 7 Gold, cast `Easterly Winds` (`twice` if Golden).

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 501;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Air Revenant", 3, 6, 5).with_tribe(Tribe::Elemental)
}

pub fn on_gold_spent(state: &mut TavernState, amount: u32, pool: &mut CardPool, rng: &mut Rng) {
    let mut total_casts = 0u32;
    for u in &mut state.board {
        if u.card_id == ID {
            u.gunpowder_gold_progress += amount;
            let triggers = u.gunpowder_gold_progress / 7;
            u.gunpowder_gold_progress %= 7;
            let mult = if u.is_golden { 2 } else { 1 };
            total_casts += triggers * mult;
        }
    }
    for _ in 0..total_casts {
        state.auras.spells_played += 1;
        let spell = spells::spell_by_name("Easterly Winds")
            .expect("Easterly Winds must exist in spell catalog");
        spells::cast_spell(state, spell, 0, pool, rng);
    }
}
