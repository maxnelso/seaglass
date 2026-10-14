//! `Passenger` (`BGDUO_114`) — Tier 1 Neutral (`2/2`), Duos only.
//! The first time your team Passes each turn, gain `+1/+2` (Golden `BGDUO_114_G`, `4/4`:
//! `+2/+4`).
//!
//! An `after_team_pass` observer: it gains the stats when the team's Passes this turn reach 1
//! (`docs/duos_tavern.md` §8.3), whoever Passed and however (a Pass by a card effect too).

use crate::cards::CardTemplate;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10101;
pub const NAME: &str = "Passenger";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 2, 1).on_after_team_pass(after_team_pass)
}

/// The team's first Pass this turn: `+1/+2` (`+2/+4` if Golden).
pub fn after_team_pass(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    if state.auras.team_passes_this_turn == 1 {
        let passenger = &mut state.board[self_idx];
        let mult = passenger.golden_mult();
        passenger.add_stats(mult, 2 * mult);
    }
}
