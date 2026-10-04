//! `Spark Snapper` (`BG36_851`) — Tier 5 Mech (`6/8`).
//!
//! Whenever you play a Mech, Magnetize a `2/3` (`4/6` if Golden) Satellite to it and improve this.

use crate::cards::{tier2, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 549;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Spark Snapper", 6, 8, 5).with_tribe(Tribe::Mech)
}

pub fn after_play_mech(
    state: &mut TavernState,
    played_tribe: Tribe,
    board_pos: usize,
    was_magnetized: bool,
) {
    if !played_tribe.matches(Tribe::Mech) || board_pos >= state.board.len() {
        return;
    }
    let mut sats: Vec<(i32, i32)> = Vec::new();
    for (idx, u) in state.board.iter_mut().enumerate() {
        if u.card_id == ID && (was_magnetized || idx != board_pos) {
            let mult = (1 + u.spark_snapper_stacks as i32) * (if u.is_golden { 2 } else { 1 });
            sats.push((2 * mult, 3 * mult));
            u.spark_snapper_stacks += 1;
        }
    }
    for (sat_atk, sat_hp) in sats {
        state.board[board_pos].add_stats(sat_atk, sat_hp);
        state.board[board_pos].magnetizations_count += 1;
        tier2::mechagnome_interpreter::after_play_or_magnetize_mech(
            state,
            Tribe::Mech,
            board_pos,
            true,
        );
    }
}
