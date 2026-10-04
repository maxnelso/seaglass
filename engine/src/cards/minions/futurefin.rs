//! `Futurefin` (`BG34_145`) — Tier 7 Murloc (`7/13`).
//!
//! At the end of your turn, give this minion's stats (double if Golden) to the left-most minion in your hand.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 703;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Futurefin", 7, 13, 7)
        .with_tribe(Tribe::Murloc)
        .on_end_of_turn(on_end_turn)
}

pub fn on_end_turn(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let unit = &state.board[self_idx];
    let mult = if unit.is_golden { 2 } else { 1 };
    let (atk, hp) = (unit.attack.max(0) * mult, unit.health.max(0) * mult);
    if atk == 0 && hp == 0 {
        return;
    }
    if let Some(target) = state.hand.iter_mut().find(|h| !h.is_spell) {
        target.add_stats(atk, hp);
    }
}
