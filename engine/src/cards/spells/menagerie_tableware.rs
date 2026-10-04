//! `Menagerie Tableware` — Tier 7 Tavern spell (`4` Gold).
//!
//! Give your minions +3/+3, plus +3/+3 per minion type among them.

use super::{select_menagerie_targets, spell};
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Menagerie Tableware`: give your minions +3/+3, plus +3/+3 per minion type among them.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, rng: &mut Rng) {
    let repeats = 1 + select_menagerie_targets(&state.board, rng).len();
    let (atk, hp) = state.auras.spell_stat_buff(3, 3);
    for _ in 0..repeats {
        for u in &mut state.board {
            u.add_stats(atk, hp);
        }
    }
}
