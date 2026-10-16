//! `Pocket Cyclone` (`BG35_MagicItem_850`) — Lesser, 0 Gold, Elemental.
//!
//! Cast Easterly Winds. At the start of each turn, cast it again.
//!
//! Picked, and at each turn start: `Easterly Winds` is cast by an effect
//! (`spells::cast_spell_by_effect`: a Tavern spell cast, which the spell-cast observers see).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130888;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(cast)
            .on_trinket_turn_start(cast),
    )]
}

fn cast(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    super::cast_easterly_winds(state, 1, pool, rng);
}
