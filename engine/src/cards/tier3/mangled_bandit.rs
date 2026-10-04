//! `Mangled Bandit` (`BG28_582`) — Tier 3 Quilboar (`3/3`).
//! **Activate (0):** Discard a card in hand to get `3` (`6` if Golden) **Blood Gems**.

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 324;
pub const NAME: &str = "Mangled Bandit";
pub const ACTIVATE_COST: u32 = 0;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 3)
        .with_tribe(Tribe::Quilboar)
        .with_activate_cost(ACTIVATE_COST)
}

pub fn on_activate(
    state: &mut TavernState,
    source_pos: usize,
    target_pos: Option<usize>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let Some(hand_idx) = target_pos else {
        return;
    };
    if hand_idx >= state.hand.len() {
        return;
    }
    let is_golden = state.board[source_pos].is_golden;
    state.discard_hand_card(hand_idx, pool, rng);
    let count = if is_golden { 6 } else { 3 };
    for _ in 0..count {
        state.add_to_hand(tokens::make_blood_gem());
    }
}
