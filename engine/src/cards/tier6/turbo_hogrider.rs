//! `Turbo Hogrider` (`BG31_323`) — Tier 6 Quilboar (`6/8`).
//!
//! After you play a Choose One card, this plays 2 (`4` if Golden) Blood Gems on all your Quilboar.

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 624;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Turbo Hogrider", 6, 8, 6)
        .with_tribe(Tribe::Quilboar)
        .on_after_friendly_choose_one(after_friendly_choose_one)
}

pub fn after_friendly_choose_one(
    state: &mut TavernState,
    self_idx: usize,
    _: &mut CardPool,
    rng: &mut Rng,
) {
    let gems = 2 * state.board[self_idx].golden_mult() as u32;
    for u in &mut state.board {
        if u.tribe.matches(Tribe::Quilboar) {
            u.play_blood_gems(gems, &state.auras);
        }
    }
    cards::resolve_pending_effects(&mut state.board, &state.auras, rng);
}
