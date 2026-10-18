//! `Mindflayer Goggles` (`BGS_Treasures_039`) — Tier 3 Darkmoon Prize.
//!
//! Steal all cards from the Tavern, then Refresh it.
//!
//! Each card in the Tavern, left to right, goes to the hand while it has room (gained, not bought:
//! no buy observers, no price; a card that can't be bought, a `Fishbait`, stays); the rest stays.
//! Then a free Refresh that the player does (`TavernState::refresh_shop_free`, `docs/tavern.md`
//! §4.1) (engine decision).

use crate::cards::spells::spell;
use crate::cards::{hooks, CardFlags, CardHooks};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 92336;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    let mut i = 0;
    while i < state.shop.len() && state.hand.len() < 10 {
        if hooks(state.shop[i].card_id).has(CardFlags::UNBUYABLE) {
            i += 1;
            continue;
        }
        let mut card = state.shop.remove(i);
        card.frozen = false;
        state.add_to_hand(card);
    }
    state.refresh_shop_free(pool, rng);
}
