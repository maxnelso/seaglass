//! `Gold-plated Compass` (`BG32_MagicItem_901`) — Greater, 0 Gold.
//!
//! Make the next 92 you buy Golden. Gain 5 free Refreshes.
//!
//! The `92` is the minion type the trinket names, assigned when it is offered
//! (`docs/trinkets.md` §3.4, confirmed). Picked: 5 free Refreshes (`auras.free_refreshes`, which
//! carry over). `trinket_minion_bought`: the next minion of that type bought (a two-type or `All`
//! minion too) becomes Golden (`Unit::make_golden`, as `Golden Touch`'s) before it reaches the
//! hand, so it makes no Triple; once (`armed`).

use crate::cards::{CardFlags, CardHooks};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 120159;

/// The free Refreshes it gives.
const REFRESHES: u32 = 5;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::TRINKET_MINION_TYPE)
            .on_trinket_bought(bought)
            .on_trinket_minion_bought(minion_bought),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.auras.free_refreshes += REFRESHES;
}

fn minion_bought(state: &mut TavernState, bought: &mut Unit) {
    let Some(tribe) = super::named_type(&state.auras, ID) else {
        return;
    };
    if bought.is_golden || !bought.tribe.matches(tribe) {
        return;
    }
    let Some(t) = super::state_mut(&mut state.auras, ID).filter(|t| t.armed == 0) else {
        return;
    };
    t.armed = 1;
    bought.make_golden();
}
