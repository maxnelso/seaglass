//! `Sharpened Sword` (`BG36_MidGameEffect_000t74`) — turns 3–6, weight 100.
//!
//! Whenever you play a card, gain +3 Attack.
//!
//! `gift_card_played`: on the board, each card the player plays from the hand (a minion or a spell,
//! as it leaves the hand; not this minion's own play) gives it +3 Attack (engine decision: the
//! board only; `docs/dark_gifts.md` §11.2). Not on Avenge minions (§5.2). A stat gift a Deathrattle
//! minion can be offered.

use crate::cards::{CardFlags, CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::CardId;
use crate::tavern::TavernState;

pub const ID: CardId = 133423;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_GIVES_STATS.union(CardFlags::GIFT_FOR_DEATHRATTLE))
            .on_gift_card_played(card_played)
            .on_gift_offerable(offerable),
    )]
}

fn card_played(state: &mut TavernState, pos: usize, _: usize) {
    state.board[pos].add_stats(3, 0);
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    !super::is_avenge(t)
}
