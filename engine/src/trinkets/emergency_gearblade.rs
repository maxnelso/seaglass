//! `Emergency Gearblade` (`BG36_MagicItem_812`) — Lesser, 0 Gold, Mech.
//!
//! At the end of your turn, cast Repair Job on your left-most Mech.
//!
//! `trinket_end_turn`: `Repair Job` is cast by an effect (`spells::cast_spell_by_effect`: a Tavern
//! spell cast, which the spell-cast observers see) on the left-most Mech on the board. Nothing
//! without a Mech.

use crate::cards::CardHooks;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133695;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_end_turn(|s, n, p, r| super::repeat_end_turn(s, n, p, r, end_turn)),
    )]
}

fn end_turn(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(pos) = state
        .board
        .iter()
        .position(|u| u.tribe.matches(Tribe::Mech))
    {
        super::cast_repair_job(state, pos, pool, rng);
    }
}
