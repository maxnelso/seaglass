//! `Blood Gem` — token spell (not a Tavern spell).
//!
//! Give a minion `+1/+1`, plus your Blood Gem bonuses.

use super::targeted_minion;
use crate::cards::{CardFlags, CardHooks};
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted_minion(cast).with_flags(CardFlags::NOT_TAVERN_SPELL | CardFlags::BLOOD_GEM)
}

/// `Blood Gem`: play a Blood Gem on a minion, once per cast (played from your hand, it is cast
/// more times with `Hot-Air Surveyor`: see [`CardFlags::BLOOD_GEM`]).
pub fn cast(
    state: &mut TavernState,
    _: &Unit,
    target: MinionSlot,
    _: &mut CardPool,
    rng: &mut Rng,
) {
    let target = match target {
        MinionSlot::Board(pos) => state.board.get_mut(pos),
        MinionSlot::Shop(idx) => state.shop.get_mut(idx),
    };
    if let Some(target) = target {
        target.play_blood_gems(1, &state.auras);
        crate::cards::resolve_pending_effects(&mut state.board, &state.auras, rng);
    }
}
