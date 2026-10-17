//! `Flaming Portrait` (`BG35_MagicItem_156`) — Greater, 7 Gold, Demon, Elemental.
//!
//! Get a Flaming Enforcer. Whenever your Flaming Enforcers trigger, also give the stats to their neighbors.
//!
//! Picked: a plain `Flaming Enforcer`. `trinket_modifies`: the stats a Flaming Enforcer gains as it
//! consumes also go to its board neighbors (`docs/trinkets.md` §11.8; the text is in its file).

use crate::cards::minions::flaming_enforcer;
use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131284;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_modifies(|card| card == flaming_enforcer::ID),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, flaming_enforcer::ID);
}
