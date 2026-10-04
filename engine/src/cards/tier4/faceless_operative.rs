//! `Faceless Operative` (`BG36_308`) — Tier 4 Aberration (`4/2`).
//!
//! When you sell this, get 2 (`4` if Golden) random Aberrations.
//! When you play one (`two` if Golden), discard the other(s).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 419;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Faceless Operative", 4, 2, 4)
        .with_tribe(Tribe::Aberration)
        .on_sell(|state, sold, pool, rng| on_sell(state, sold, pool, rng))
}

pub fn on_sell(state: &mut TavernState, sold: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    let count = if sold.is_golden { 4 } else { 2 };
    let keep = if sold.is_golden { 2 } else { 1 };
    let grp = state.next_willbreaker_group;
    state.next_willbreaker_group += 1;
    for _ in 0..count {
        if state.hand.len() >= 10 {
            break;
        }
        if let Some(mut drawn) =
            pool.draw_by_tribe(Tribe::Aberration, Some(ID), state.tavern_tier, rng)
        {
            drawn.willbreaker_group = grp;
            drawn.willbreaker_remaining = keep;
            state.add_to_hand(drawn);
        }
    }
}
