//! `Well Wishes` (`BGDUO_120t`) — Spellcraft token spell of `Well Wisher`, Duos only.
//!
//! Pass a non-Golden minion (except Well Wisher).
//!
//! It is cast on a friendly non-Golden minion that isn't a `Well Wisher`: the minion leaves the
//! board, without going back to the pool, and is Passed with its stats, a free Pass that counts
//! as the team's (`docs/duos_tavern.md` §8.3). A Golden `Well Wisher`'s spell does the same. A
//! temporary token spell, not a Tavern spell.

use super::{targeted, SPELLCRAFT};
use crate::cards::minions::well_wisher;
use crate::cards::{self, CardHooks};
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
        .on_spell_target_filter(|unit| !unit.is_golden && unit.card_id != well_wisher::ID)
        .with_flags(SPELLCRAFT)
}

/// `Well Wishes`: Pass the friendly minion.
pub fn cast(state: &mut TavernState, _: &Unit, board_pos: usize, _: &mut CardPool, _: &mut Rng) {
    cards::pass_board_minion(state, board_pos);
}
