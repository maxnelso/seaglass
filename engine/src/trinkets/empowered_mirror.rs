//! `Empowered Mirror` (`BGDUO_MagicItem_010`) — Lesser, 3 Gold, Duos only.
//!
//! Become a copy of your teammate's Lesser Trinket.
//!
//! `trinket_teammate`: as soon as the teammate owns a Lesser trinket (other than an `Empowered
//! Mirror`), the Mirror becomes a copy of it (`trinkets::replace`, `docs/trinkets.md` §3.5): a
//! fresh one, naming the same minion type, whose own text resolves. Checked after each of the
//! team's actions and at the team's turn start, so a teammate who picks theirs later is copied
//! then **(engine decision)**.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};
use crate::trinkets::TeammateView;

pub const ID: CardId = 117941;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_teammate(teammate))]
}

fn teammate(state: &mut TavernState, mate: &TeammateView<'_>, pool: &mut CardPool, rng: &mut Rng) {
    super::become_teammates(state, ID, false, mate, pool, rng);
}
