//! `Souvenir Stand` (`BG30_MagicItem_888`) — Lesser, 0 Gold.
//!
//! When you buy a Greater Trinket, this transforms into a copy of it.
//!
//! `trinket_after_pick`: a Greater trinket picked from an offer (the turn 9 one, `Trip
//! Vouchers`', ...) makes the Stand a copy of it (`trinkets::replace`, `docs/trinkets.md` §3.5):
//! a fresh one, naming the same minion type, whose own text resolves too **(engine decision)**.
//! The player then owns two copies of it, each with its own state (§2.3).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 112895;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_pick(after_pick))]
}

fn after_pick(state: &mut TavernState, picked: CardId, pool: &mut CardPool, rng: &mut Rng) {
    if !super::data(picked).is_some_and(|t| t.greater) {
        return;
    }
    let tribe = super::state(&state.auras, picked).map_or_else(Default::default, |t| t.tribe);
    super::replace(state, ID, picked, tribe, pool, rng);
}
