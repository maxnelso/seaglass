//! `Mechagnome Interpreter` (`BG31_177`) — Tier 2 Mech (`3/1`).
//! Whenever you play or **Magnetize** a Mech, give it `+3/+1` (`+6/+2` if Golden).

use crate::cards::{CardTemplate, Played};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 218;
pub const NAME: &str = "Mechagnome Interpreter";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 1, 2)
        .with_tribe(Tribe::Mech)
        .on_after_friendly_play(after_friendly_play)
}

pub fn after_friendly_play(
    state: &mut TavernState,
    self_idx: usize,
    played: &Played,
    _: &mut CardPool,
    _: &mut Rng,
) {
    if !played.tribe.matches(Tribe::Mech) || played.board_pos >= state.board.len() {
        return;
    }
    let mult = state.board[self_idx].golden_mult();
    state.board[played.board_pos].add_stats(3 * mult, mult);
}
