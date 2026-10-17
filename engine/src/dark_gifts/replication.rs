//! `Replication` (`BG36_MidGameEffect_000t18`) — turns 4–7, weight 100.
//!
//! At the end of every 2 turns, get a plain copy of this.
//!
//! `gift_end_of_turn`: on the board, every 2nd end of turn (`counter`, advanced once per end of
//! turn; the payoff repeats with the end-of-turn repeat count), a plain copy of it
//! (`cards::instantiate_plain_copy`: no gifts) to the hand (lost if the hand is full, as a minion's
//! own card text; engine decision; `docs/dark_gifts.md` §11.2). A gift for Battlecry minions
//! (§5.1).

use crate::cards::{self, CardFlags, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 132445;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_FOR_BATTLECRY)
            .on_gift_end_of_turn(end_of_turn),
    )]
}

fn end_of_turn(
    state: &mut TavernState,
    pos: usize,
    gift_idx: usize,
    repeats: u32,
    _: &mut CardPool,
    _: &mut Rng,
) {
    if !super::every_two_turns(&mut state.board[pos].dark_gifts[gift_idx]) {
        return;
    }
    let copy = cards::instantiate_plain_copy(&state.board[pos]);
    for _ in 0..repeats {
        state.add_to_hand(copy.clone());
    }
}
