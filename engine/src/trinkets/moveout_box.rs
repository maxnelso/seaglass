//! `Moveout Box` (`BGDUO_MagicItem_008`) — Greater, 10 Gold, Duos only.
//!
//! Skip this turn. At the end of it, replace your warband with a copy of your teammate's.
//!
//! Picked, it remembers the turn (`counter`, `armed` 1). `trinket_skips_turn`: that turn the
//! player can only end it (and resolve prompts), as a Hero Power that skips a turn. Once the
//! team's turn is over (both players ended it; `trinket_teammate`), the player's board is
//! replaced by exact copies of the teammate's board as it is then, their own minions going back
//! to the pool **(engine decision**: the teammate's final warband**)**. Once.

use crate::cards::CardHooks;
use crate::model::{CardId, TrinketState};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};
use crate::trinkets::TeammateView;

pub const ID: CardId = 117939;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_skips_turn(skips_turn)
            .on_trinket_teammate(teammate),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let turn = state.turn as i32;
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        (t.counter, t.armed) = (turn, 1);
    }
}

fn skips_turn(t: &TrinketState, state: &TavernState) -> bool {
    t.armed == 1 && t.counter == state.turn as i32
}

fn teammate(state: &mut TavernState, mate: &TeammateView<'_>, pool: &mut CardPool, _: &mut Rng) {
    let turn = state.turn as i32;
    let Some(t) = super::state_mut(&mut state.auras, ID) else {
        return;
    };
    if !(mate.turn_over && t.armed == 1 && t.counter == turn) {
        return;
    }
    t.armed = 0;
    for old in std::mem::take(&mut state.board) {
        pool.return_unit(&old);
    }
    state.board = mate.state.board.clone();
    state.sync_all_auras();
}
