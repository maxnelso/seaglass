//! `Spark Snapper` (`BG36_851`) — Tier 5 Mech (`6/8`).
//!
//! Whenever you play a Mech, Magnetize a `2/3` (`4/6` if Golden) Satellite to it and improve this.

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

pub fn after_friendly_play(
    state: &mut TavernState,
    self_idx: usize,
    played: &Played,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if played.magnetized
        || !played.tribe.matches(Tribe::Mech)
        || played.board_pos >= state.board.len()
    {
        return;
    }
    let snapper = &mut state.board[self_idx];
    let mult = (1 + snapper.spark_snapper_stacks as i32) * (if snapper.is_golden { 2 } else { 1 });
    snapper.spark_snapper_stacks += 1;
    let mut satellite = tokens::make_satellite(false);
    satellite.attack = 2 * mult;
    satellite.health = 3 * mult;
    cards::apply_magnetization(state, &satellite, played.board_pos, pool, rng);
}
