//! `Shrine of Evolution` (`BG32_MagicItem_400`) — Lesser, 0 Gold.
//!
//! Transform all your minions into random Tier 4 minions.
//!
//! Picked: each minion on the board, left to right, is transformed into a random Tier 4 minion
//! drawn from the pool (`CardPool::draw_discover_options`, one draw each, as `Robust Evolution`'s
//! transform; the highest lower Tier with copies left if none), its own copies going back to the
//! pool: a plain minion with its base stats, never Golden (**engine decision**). The auras resync
//! and Triples resolve.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 111427;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    for pos in 0..state.board.len() {
        let Some(new) = pool.draw_discover_options(4, 1, rng).pop() else {
            break;
        };
        let old = std::mem::replace(&mut state.board[pos], new);
        pool.return_unit(&old);
        state.board[pos].id = old.id;
    }
    state.sync_all_auras();
}
