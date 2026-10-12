//! `Spark Snapper` (`BG36_851`) — Tier 5 Mech (`6/8`).
//!
//! Whenever you play a Mech, Magnetize a `2/3` (`4/6` if Golden) Satellite to it and improve this.
//!
//! Playing a Mech by Magnetizing it onto another Mech counts (the Satellite then goes to that
//! Mech). The Satellite is a Magnetization (`Mechagnome Interpreter`, `Polarizing Beatboxer` see
//! it) but not a play. Nor is another card's Magnetization (a `Glambot` Satellite): it neither
//! triggers nor improves this.

use crate::cards::{self, tokens, CardTemplate, Played};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 549;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Spark Snapper", 6, 8, 5)
        .with_tribe(Tribe::Mech)
        .on_after_friendly_play(after_friendly_play)
}

/// A Mech played from hand (to the board, or Magnetized onto a minion at `played.board_pos`).
pub fn after_friendly_play(
    state: &mut TavernState,
    self_idx: usize,
    played: &Played,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if !played.from_hand
        || !played.tribe.matches(Tribe::Mech)
        || played.board_pos >= state.board.len()
    {
        return;
    }
    let snapper = &mut state.board[self_idx];
    let mult = (1 + snapper.stacks) * (if snapper.is_golden { 2 } else { 1 });
    snapper.stacks += 1;
    let mut satellite = tokens::make_satellite(false);
    satellite.attack = 2 * mult;
    satellite.health = 3 * mult;
    cards::magnetize(state, &mut satellite, played.board_pos, pool, rng);
}
