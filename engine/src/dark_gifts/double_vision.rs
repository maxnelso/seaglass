//! `Double Vision` (`BG36_MidGameEffect_000t11`) — turns 5–, weight 100.
//!
//! Get an extra copy of this.
//!
//! `gift_obtained`: when it is Discovered, a copy of it as offered, without `Double Vision`, goes
//! to the hand (not from the pool; engine decision), or waits for room in a full hand (confirmed;
//! `docs/dark_gifts.md` §2.2, §11.2). A gift for Battlecry minions (§5.1).

use crate::cards::{CardFlags, CardHooks};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 132485;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_FOR_BATTLECRY)
            .on_gift_obtained(obtained),
    )]
}

fn obtained(state: &mut TavernState, card: &Unit, _: &mut CardPool, _: &mut Rng) {
    let mut copy = card.clone();
    copy.id = 0;
    copy.dark_gifts.retain(|g| g.card_id != ID);
    state.add_to_hand_or_wait(copy);
}
