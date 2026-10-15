//! `Kith'ix` (`BG36_HERO_002`) — Armor 8 (Duos 8).
//!
//! Hero Power: `Dark Ritual` (`BG36_HERO_002p`), 2 Gold: Get 2 random minions. When you play one, discard the other.
//!
//! Active, untargeted, once per turn: 2 random minions of the player's Tier or lower from the pool
//! go to the hand as one group of a single play (the `Faceless Operative` group,
//! `docs/tavern.md` §6): playing or Passing one discards the other. It needs room in the hand.

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 134504;
pub const POWER: CardId = 134505;

/// Minions it gives.
const MINIONS: usize = 2;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(dark_ritual)
            .on_hero_power_usable(super::hand_has_room),
    )]
}

/// Using it: 2 random minions, one play between them.
fn dark_ritual(state: &mut TavernState, _: Option<usize>, pool: &mut CardPool, rng: &mut Rng) {
    let group = state.next_willbreaker_group;
    state.next_willbreaker_group += 1;
    for _ in 0..MINIONS {
        let Some(mut minion) = pool.draw_from_pool(state.tavern_tier, rng) else {
            break;
        };
        minion.willbreaker_group = group;
        minion.willbreaker_remaining = 1;
        state.add_pool_card_to_hand(minion, pool);
    }
}
