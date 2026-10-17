//! `Time Turning` (`BG36_MidGameEffect_000t21`) — turns 5–8, weight 200.
//!
//! This minion's end of turn effects also trigger at start of turn.
//!
//! `gift_turn_start`: on the board, at the start of each turn, after its other start-of-turn
//! effects, its card's end of turn effect (`end_of_turn`) and its other gifts' (`gift_end_of_turn`)
//! trigger once each (`docs/dark_gifts.md` §11.2). Only on minions with end of turn effects (§5.2).

use crate::cards::{CardHooks, CardTemplate};
use crate::dark_gifts::{OfferCtx, Zone};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 132448;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_gift_turn_start(turn_start)
            .on_gift_offerable(offerable),
    )]
}

fn turn_start(
    state: &mut TavernState,
    zone: Zone,
    pos: usize,
    gift_idx: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if zone != Zone::Board {
        return;
    }
    let card_id = state.board[pos].card_id;
    if let Some(end_of_turn) = crate::cards::hooks(card_id).end_of_turn {
        end_of_turn(state, pos, pool, rng);
    }
    if state.board.get(pos).is_some_and(|u| u.card_id == card_id) {
        super::end_of_turn_of(state, pos, gift_idx, pool, rng);
    }
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    super::is_end_of_turn(t)
}
