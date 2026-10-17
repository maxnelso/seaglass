//! `Hackerfin Portrait` (`BG32_MagicItem_925`) — Greater, 6 Gold, Murloc.
//!
//! Get a Hackerfin. At the end of each turn, trigger your Hackerfins' Battlecries.
//!
//! Picked: a plain `Hackerfin`. `trinket_end_turn`: each Hackerfin on the board, left to right,
//! triggers its Battlecry (`cards::trigger_board_battlecry`, as `Young Murk-Eye`'s); repeated by
//! the end-of-turn repeat count.

use crate::cards::minions::hackerfin;
use crate::cards::{self, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 121109;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_end_turn(end_turn),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, hackerfin::ID);
}

fn end_turn(state: &mut TavernState, repeats: u32, pool: &mut CardPool, rng: &mut Rng) {
    super::repeat_end_turn(state, repeats, pool, rng, trigger);
}

fn trigger(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let mut pos = 0;
    while pos < state.board.len() {
        if state.board[pos].card_id == hackerfin::ID {
            cards::trigger_board_battlecry(state, pos, pool, rng);
        }
        pos += 1;
    }
}
