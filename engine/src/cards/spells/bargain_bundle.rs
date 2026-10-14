//! `Bargain Bundle` (`BG31_242`) — Tier 5 Tavern spell (`5` Gold), Duos only.
//!
//! Discover a minion. Your teammate gets the other options.
//!
//! It Discovers among 3 minions of the player's Tavern Tier or lower from the pool. Its options
//! are marked (`Unit::unchosen_to_teammate`): once one is chosen, the others go to the teammate
//! (`Card` gifts, not Passes) instead of back to the pool (`docs/duos_tavern.md` §8.3). Only Duos
//! shops offer it.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Bargain Bundle`: Discover a minion; the teammate gets the other options.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    let mut opts = pool.draw_discover_filtered(state.tavern_tier, 3, |_| true, rng);
    for opt in &mut opts {
        state.apply_global_unit_auras(opt);
        opt.unchosen_to_teammate = true;
    }
    state.push_discover(opts);
}
