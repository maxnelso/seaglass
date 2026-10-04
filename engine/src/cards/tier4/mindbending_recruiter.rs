//! `Mindbending Recruiter` (`BG36_312`) — Tier 4 Aberration (`6/2`).
//!
//! Activate (0): Discard a card to get a (`2` if Golden) random Aberration(s).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 442;
pub const ACTIVATE_COST: u32 = 0;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Mindbending Recruiter", 6, 2, 4)
        .with_tribe(Tribe::Aberration)
        .with_activate_cost(ACTIVATE_COST)
}

pub fn on_activate(
    state: &mut TavernState,
    source_pos: usize,
    target_pos: Option<usize>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let Some(t_pos) = target_pos else {
        return;
    };
    if t_pos >= state.hand.len() {
        return;
    }
    let is_golden = state.board[source_pos].is_golden;
    let count = if is_golden { 2 } else { 1 };
    state.discard_hand_card(t_pos, pool, rng);
    for _ in 0..count {
        if state.hand.len() >= 10 {
            break;
        }
        if let Some(drawn) =
            pool.draw_by_tribe(Tribe::Aberration, Some(ID), state.tavern_tier, rng)
        {
            state.add_to_hand(drawn);
        }
    }
}
