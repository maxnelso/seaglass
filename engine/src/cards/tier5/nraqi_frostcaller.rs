//! `N'raqi Frostcaller` (`BG36_300`) — Tier 5 Aberration (`6/3`).
//!
//! Activate (0): Discard a card for your Tavern spells to give an extra `+1/+1` (`+2/+2` if Golden) this game.

use crate::cards::{ActivateTargetKind, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 535;
pub const ACTIVATE_COST: u32 = 0;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "N'raqi Frostcaller", 6, 3, 5)
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
    let bonus = if state.board[source_pos].is_golden { 2 } else { 1 };
    state.discard_hand_card(hand_idx, pool, rng);
    state.auras.spell_bonus_atk += bonus;
    state.auras.spell_bonus_hp += bonus;
}
