//! `The Good Stuff` (`BGS_Treasures_013`) — Tier 1 Darkmoon Prize.
//!
//! Give minions in the Tavern +1/+1 this game.
//!
//! The minions in the Tavern gain +1/+1 now, and every minion the Tavern offers later has it
//! (`tavern_all_atk` / `tavern_all_hp`, as `Staff of Enrichment`). Not a Tavern spell: the Tavern
//! spell bonuses don't apply.

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 65254;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    crate::trinkets::buff_tavern_minions(state, 1, 1);
}
