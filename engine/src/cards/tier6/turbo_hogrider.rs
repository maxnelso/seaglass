//! `Turbo Hogrider` (`BG31_323`) — Tier 6 Quilboar (`6/8`).
//!
//! After you play a Choose One card, this plays 2 (`4` if Golden) Blood Gems on all your Quilboar.

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 624;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Turbo Hogrider", 6, 8, 6).with_tribe(Tribe::Quilboar)
}

pub fn after_play_choose_one(state: &mut TavernState, rng: &mut Rng) {
    let mut total_gems = 0u32;
    for u in &state.board {
        if u.card_id == ID {
            total_gems += if u.is_golden { 4 } else { 2 };
        }
    }
    if total_gems == 0 {
        return;
    }
    for u in &mut state.board {
        if u.tribe.matches(Tribe::Quilboar) {
            u.play_blood_gems(total_gems, &state.auras);
        }
    }
    cards::resolve_roogug_procs(&mut state.board, &state.auras, rng);
}
