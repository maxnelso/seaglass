//! `Rewinder Portrait` (`BG30_MagicItem_868`) — Lesser, 4 Gold, Demon.
//!
//! Get a Soul Rewinder and a Wrath Weaver. Your Soul Rewinders also gain Attack.
//!
//! Picked: a plain `Soul Rewinder`, then a plain `Wrath Weaver`. `trinket_modifies`: a Soul
//! Rewinder gains as much Attack as Health (`docs/trinkets.md` §11.8; the text is in its file).

use crate::cards::minions::{soul_rewinder, wrath_weaver};
use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 110975;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_modifies(|card| card == soul_rewinder::ID),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, soul_rewinder::ID);
    super::give_minion(state, wrath_weaver::ID);
}
