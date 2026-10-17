//! `Echoing Voice` (`BG36_MidGameEffect_000t10`) — turns 6–, weight 100.
//!
//! At the end of your turn, trigger this minion's Battlecries.
//!
//! `gift_end_of_turn`: on the board, at the end of each turn (each of its repeats), its Battlecry
//! triggers (`cards::trigger_board_battlecry`). Only on Battlecry minions, a gift for them
//! (`docs/dark_gifts.md` §5).

use crate::cards::{self, CardFlags, CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 132208;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_FOR_BATTLECRY)
            .on_gift_end_of_turn(end_of_turn)
            .on_gift_offerable(offerable),
    )]
}

fn end_of_turn(
    state: &mut TavernState,
    pos: usize,
    _: usize,
    repeats: u32,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let card_id = state.board[pos].card_id;
    for _ in 0..repeats {
        if state.board.get(pos).is_none_or(|u| u.card_id != card_id) {
            return;
        }
        cards::trigger_board_battlecry(state, pos, pool, rng);
    }
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    super::is_battlecry(t)
}
