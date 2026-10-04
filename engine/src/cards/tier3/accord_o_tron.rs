//! `Accord-o-Tron` (`BG26_147`) — Tier 3 Mech (`3/3`).
//! **Magnetic**. At the start of your turn, gain `1` (`2` if Golden) Gold.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe, Unit};
use crate::tavern::{CardPool, TavernState};
use crate::rng::Rng;

pub const ID: CardId = 302;
pub const NAME: &str = "Accord-o-Tron";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 3)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::Magnetic)
        .on_turn_start(on_start_turn)
        .on_magnetize_transfer(on_magnetize_transfer)
}

pub fn on_start_turn(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    state.gold += if state.board[self_idx].is_golden { 2 } else { 1 };
}

/// Magnetized: the target gains this start-of-turn effect.
pub fn on_magnetize_transfer(source: &Unit, target: &mut Unit) {
    target.sot_gold_bonus += if source.is_golden { 2 } else { 1 };
}
