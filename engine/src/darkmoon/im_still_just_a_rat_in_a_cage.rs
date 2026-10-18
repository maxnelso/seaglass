//! `I'm Still Just a Rat in a Cage` (`BGS_Treasures_018`) — Tier 2 Darkmoon Prize.
//!
//! Give a minion +2 Attack, then double its Attack.
//!
//! "A minion": a friendly one or one in the Tavern (`SpellTarget::Minion`). It gains +2 Attack,
//! then its Attack again.

use crate::cards::spells::targeted_minion;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

pub const ID: CardId = 65262;

/// The Attack it gives before the doubling.
const ATTACK: i32 = 2;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, targeted_minion(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, target: MinionSlot, _: &mut CardPool, _: &mut Rng) {
    if let Some(unit) = state.minion_mut(target) {
        unit.add_stats(ATTACK, 0);
        let attack = unit.attack;
        unit.add_stats(attack, 0);
    }
}
