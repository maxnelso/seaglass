//! `Blood Gem` — token spell (not a Tavern spell).
//!
//! Give a minion `+1/+1`, plus your Blood Gem bonuses.
//!
//! A `Jewelry Box` gem (its `counter` names a keyword: 1 Taunt, 2 Divine Shield, 3 Reborn) also
//! gives that keyword to a Quilboar it is played on, each cast (`docs/trinkets.md` §11.2).

use super::targeted_minion;
use crate::cards::{CardFlags, CardHooks};
use crate::model::{Keyword, Tribe, Unit};
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
    card: &Unit,
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
        if let Some(keyword) = gem_keyword(card).filter(|_| target.tribe.matches(Tribe::Quilboar)) {
            target.apply_keyword(keyword, false);
        }
        crate::cards::resolve_pending_effects(&mut state.board, &state.auras, rng);
    }
}

/// The keyword a `Jewelry Box` gem gives a Quilboar, from its `counter` (none for a plain gem).
pub fn gem_keyword(card: &Unit) -> Option<Keyword> {
    match card.counter {
        1 => Some(Keyword::Taunt),
        2 => Some(Keyword::DivineShield),
        3 => Some(Keyword::Reborn),
        _ => None,
    }
}
