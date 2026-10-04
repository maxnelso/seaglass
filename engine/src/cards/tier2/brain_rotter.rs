//! `Brain Rotter` (`BG36_099`) — Tier 2 Aberration (`3/4`).
//! **Activate (0):** Discard a card in hand to give your **Deity** `+2/+2` (`+4/+4` if Golden).

use crate::cards::{ActivateTargetKind, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 203;
pub const NAME: &str = "Brain Rotter";
pub const ACTIVATE_COST: u32 = 0;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 4, 2)
        .with_tribe(Tribe::Aberration)
        .with_activate_cost(ACTIVATE_COST)
        .with_activate_target(ActivateTargetKind::HandCard)
        .on_activate(|state, source_pos, target_pos, pool, rng| {
            on_activate(state, source_pos, target_pos, pool, rng)
        })
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
    let mult = if is_golden { 2 } else { 1 };
    state.auras.deity.attack += 2 * mult;
    state.auras.deity.health += 2 * mult;
}
