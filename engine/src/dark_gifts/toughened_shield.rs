//! `Toughened Shield` (`BG36_MidGameEffect_000t75`) — turns 3–6, weight 100.
//!
//! Whenever you play a card, gain +3 Health.
//!
//! `gift_card_played`: on the board, each card the player plays from the hand (a minion or a spell,
//! as it leaves the hand; not this minion's own play) gives it +3 Health (engine decision: the
//! board only; `docs/dark_gifts.md` §11.2). Not in a Quilboar (or Naga) lobby (§5.1 rule 5). A stat
//! gift.

use crate::cards::{CardFlags, CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::CardId;
use crate::model::Tribe;
use crate::tavern::TavernState;
pub const ID: CardId = 133424;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_GIVES_STATS)
            .on_gift_card_played(card_played)
            .on_gift_offerable(offerable),
    )]
}

fn card_played(state: &mut TavernState, pos: usize, _: usize) {
    state.board[pos].add_stats(0, 3);
}

fn offerable(ctx: &OfferCtx<'_>, _: &CardTemplate) -> bool {
    !ctx.lobby_has(Tribe::Quilboar) && !ctx.lobby_has(Tribe::Naga)
}
