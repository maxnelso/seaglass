//! `Maldraxxus Dagger` (`BG36_MagicItem_370`) — Greater, 6 Gold.
//!
//! Discover a plain copy of a friendly minion with a Dark Gift. Repeat at the start of every 2 turns. (2 turns left!)
//!
//! As picked, and at every 2nd turn start after (`counter`): an offer of up to 3 different board
//! minions (distinct cards, one `rng.below(k)` each), each a plain copy taken out of the pool like
//! a Discover's minion (a card with no copy left in the pool isn't offered; the unchosen ones go
//! back) with a different gift by the turn's rules; a minion no gift fits is dropped (engine
//! decision; `docs/dark_gifts.md` §12.3). Offered only in games with Dark Gifts
//! (`CardFlags::DARK_GIFTS`).

use crate::cards::{self, CardFlags, CardHooks};
use crate::dark_gifts::{self, OfferRules};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133713;

/// The turns between two offers.
const EVERY: i32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::DARK_GIFTS)
            .on_trinket_bought(discover)
            .on_trinket_turn_start(turn_start),
    )]
}

/// Every 2nd turn start after the pick: another offer.
fn turn_start(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let Some(t) = super::state_mut(&mut state.auras, ID) else {
        return;
    };
    t.counter += 1;
    if t.counter < EVERY {
        return;
    }
    t.counter = 0;
    discover(state, pool, rng);
}

/// The offer: plain copies of up to 3 different board minions, each with a gift.
fn discover(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let mut ids: Vec<CardId> = Vec::new();
    for unit in &state.board {
        if !ids.contains(&unit.card_id) && pool.remaining_copies(unit.card_id) > 0 {
            ids.push(unit.card_id);
        }
    }
    let mut copies = Vec::with_capacity(dark_gifts::OFFER_COUNT);
    while copies.len() < dark_gifts::OFFER_COUNT && !ids.is_empty() {
        let id = ids.remove(rng.below(ids.len()));
        if let Some(t) = cards::template(id) {
            pool.take_copy(id);
            copies.push(t.instantiate());
        }
    }
    let rules = OfferRules::button(state.turn);
    let options = dark_gifts::gift_options(state, pool, rng, copies, &rules);
    state.push_discover(options);
}
