//! `Menagerie Tableware` — Tier 7 Tavern spell (`4` Gold).
//!
//! Give your minions +3/+3, plus +3/+3 per minion type among them. The types are counted as for
//! `Misplaced Tea Set`: the most types your minions can each stand for at once (a `Treasure
//! Parrot` and a `Flittering Bat` count 2).

use super::{menagerie_type_count, spell};
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Menagerie Tableware`: give your minions +3/+3, plus +3/+3 per minion type among them.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    let repeats = 1 + menagerie_type_count(&state.board);
    let (atk, hp) = state.auras.spell_stat_buff(3, 3);
    for _ in 0..repeats {
        for u in &mut state.board {
            u.add_stats(atk, hp);
        }
    }
}
