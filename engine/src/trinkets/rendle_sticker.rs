//! `Rendle Sticker` (`BG32_MagicItem_817`) — Lesser, 0 Gold.
//!
//! Steal the highest-Tier card from the Tavern. At the end of each turn, repeat this.
//!
//! Picked, and at the end of each turn (`trinket_end_turn`, after the cards' end-of-turn
//! triggers): the card of the highest Tier in the Tavern (a minion or a spell; ties are broken at
//! random, one `rng.below(k)` among the tied: confirmed) leaves it for the hand, as it is (not a
//! buy: no buy triggers, nothing paid), and waits for room in a full hand. Nothing from an empty
//! Tavern.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 120154;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(steal)
            .on_trinket_end_turn(|s, n, p, r| super::repeat_end_turn(s, n, p, r, steal)),
    )]
}

fn steal(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    let Some(best) = state.shop.iter().map(|u| u.tavern_tier).max() else {
        return;
    };
    let tied: Vec<usize> = (0..state.shop.len())
        .filter(|&i| state.shop[i].tavern_tier == best)
        .collect();
    let mut card = state.shop.remove(super::pick_tied(&tied, rng));
    card.frozen = false;
    state.add_to_hand_or_wait(card);
}
