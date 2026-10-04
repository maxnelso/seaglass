//! `Lullabot` (`BG26_146`) — Tier 1 Mech (`2/2`).
//! **Magnetic**. At the end of your turn, gain `+1` (`+2` if Golden) Health.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe, Unit};
use crate::tavern::{CardPool, TavernState};
use crate::rng::Rng;

pub const ID: CardId = 111;
pub const NAME: &str = "Lullabot";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 2, 1)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::Magnetic)
        .on_end_of_turn(on_end_turn)
        .on_magnetize_transfer(on_magnetize_transfer)
}

pub fn on_end_turn(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let unit = &mut state.board[self_idx];
    unit.add_stats(0, if unit.is_golden { 2 } else { 1 });
}

/// Magnetized: the target gains this end-of-turn effect.
pub fn on_magnetize_transfer(source: &Unit, target: &mut Unit) {
    target.eot_health_bonus += if source.is_golden { 2 } else { 1 };
}
