//! `Demonic Bloodletter` (`BG36_MagicItem_800`) — Lesser, 3 Gold, Demon.
//!
//! Whenever you cast a Tavern spell, give minions in the Tavern +1/+1 this game.
//!
//! `trinket_spell_cast`: each Tavern spell cast raises the Tavern's bonus (`auras.tavern_all_atk`
//! / `tavern_all_hp`, as `Staff of Enrichment`'s) by +1/+1, and the minions in the Tavern now gain
//! it (`trinkets::buff_tavern_minions`).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133060;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_spell_cast(spell_cast))]
}

fn spell_cast(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::buff_tavern_minions(state, 1, 1);
}
