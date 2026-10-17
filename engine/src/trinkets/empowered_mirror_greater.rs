//! `Empowered Mirror (Greater)` (`BGDUO_MagicItem_010t`) — Greater, 3 Gold, Duos only.
//!
//! Become a copy of your teammate's Greater Trinket.
//!
//! As the Lesser one, for the teammate's Greater trinket (`docs/trinkets.md` §3.5).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};
use crate::trinkets::TeammateView;

pub const ID: CardId = 117942;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_teammate(teammate))]
}

fn teammate(state: &mut TavernState, mate: &TeammateView<'_>, pool: &mut CardPool, rng: &mut Rng) {
    super::become_teammates(state, ID, true, mate, pool, rng);
}
