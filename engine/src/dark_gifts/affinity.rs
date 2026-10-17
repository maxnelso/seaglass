//! `Affinity` (`BG36_MidGameEffect_000t82`) — turns 3–4, weight 100.
//!
//! At the end of every 2 turns, get a random minion of this type.
//!
//! `gift_end_of_turn`: on the board, every 2nd end of turn (`counter`, advanced once per end of
//! turn; the payoff repeats with the end-of-turn repeat count), a random minion of one of its types
//! (one draw among its types, then one among the game's catalog of the player's Tier or lower) to
//! the hand, or waiting for room in a full hand (confirmed; `docs/dark_gifts.md` §2.2, §11.2). Only on minions with a type (§5.2).

use crate::cards::{self, CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::{CardId, SINGLE_TRIBES};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133860;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_gift_end_of_turn(end_of_turn)
            .on_gift_offerable(offerable),
    )]
}

fn end_of_turn(
    state: &mut TavernState,
    pos: usize,
    gift_idx: usize,
    repeats: u32,
    _: &mut CardPool,
    rng: &mut Rng,
) {
    if !super::every_two_turns(&mut state.board[pos].dark_gifts[gift_idx]) {
        return;
    }
    let tribe = state.board[pos].tribe;
    let types: Vec<_> = SINGLE_TRIBES
        .into_iter()
        .filter(|&t| tribe.has_single(t))
        .collect();
    for _ in 0..repeats {
        if types.is_empty() {
            return;
        }
        let wanted = types[rng.below(types.len())];
        let tier = state.tavern_tier;
        let pool: Vec<&CardTemplate> = cards::game_templates(&state.auras)
            .iter()
            .filter(|t| t.tavern_tier <= tier && t.tribe.has_single(wanted))
            .collect();
        if !pool.is_empty() {
            let card = pool[rng.below(pool.len())].instantiate();
            state.add_to_hand_or_wait(card);
        }
    }
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    super::has_type(t)
}
