//! `Steady Growth` (`BG36_MidGameEffect_000t51`) — turns 3–5, weight 100.
//!
//! At the end of your turn, gain +{1}/+{2}.
//!
//! `gift_end_of_turn`: on the board, at the end of each turn (each of its repeats), it gains
//! `+1/+2` if the gift was given on turn 3 or before, `+2/+2` on turn 4, `+3/+3` on turn 5, `+4/+4`
//! later (the post's numbers; the strength is the gift's `stacks`, its offering turn;
//! `docs/dark_gifts.md` §11.2). A stat gift.

use crate::cards::{CardFlags, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 132733;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_GIVES_STATS)
            .on_gift_end_of_turn(end_of_turn),
    )]
}

/// The stats it gives each end of turn, for the turn the gift was given on.
pub fn growth(turn: i32) -> (i32, i32) {
    match turn {
        ..=3 => (1, 2),
        4 => (2, 2),
        5 => (3, 3),
        _ => (4, 4),
    }
}

fn end_of_turn(
    state: &mut TavernState,
    pos: usize,
    gift_idx: usize,
    repeats: u32,
    _: &mut CardPool,
    _: &mut Rng,
) {
    let (atk, hp) = growth(state.board[pos].dark_gifts[gift_idx].stacks);
    for _ in 0..repeats {
        state.board[pos].add_stats(atk, hp);
    }
}
