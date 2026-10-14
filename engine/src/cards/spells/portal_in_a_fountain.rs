//! `Portal in a Fountain` (`BG31_243`) — Tier 3 Tavern spell (`3` Gold), Duos only.
//!
//! Pass a minion in the Tavern.
//!
//! It is cast on a minion in the Tavern, and only there (`SpellTarget::TavernMinion`): the minion
//! leaves the shop, without going back to the pool, and is Passed with its stats, a free Pass
//! that counts as the team's (`docs/duos_tavern.md` §8.3). Only Duos shops offer it.

use super::targeted_tavern_minion;
use crate::cards::{self, CardHooks};
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted_tavern_minion(cast)
}

/// `Portal in a Fountain`: Pass the minion in the Tavern.
pub fn cast(state: &mut TavernState, _: &Unit, target: MinionSlot, _: &mut CardPool, _: &mut Rng) {
    if let MinionSlot::Shop(idx) = target {
        cards::pass_shop_minion(state, idx);
    }
}
