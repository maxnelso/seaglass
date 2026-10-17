//! `Gold Pendant` (`BG32_MagicItem_951`) — Lesser, 1 Gold, Pirate.
//!
//! Make a random friendly minion from Tier 4 or below Golden. At the start of each turn, repeat this.
//!
//! Picked, and at each turn start: a random minion on the board of Tier 4 or lower that isn't
//! Golden (one `rng.below(k)`) becomes Golden (`Unit::make_golden`, as `Golden Touch`'s: it still
//! holds one copy of the pool). Nothing without one.

use crate::cards::{self, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 120809;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(goldenize)
            .on_trinket_turn_start(goldenize),
    )]
}

fn goldenize(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    let candidates: Vec<usize> = (0..state.board.len())
        .filter(|&i| !state.board[i].is_golden && state.board[i].tavern_tier <= 4)
        .collect();
    if candidates.is_empty() {
        return;
    }
    let pick = candidates[rng.below(candidates.len())];
    state.board[pick].make_golden();
    cards::sync_board_unit_auras(&mut state.board[pick], &state.auras);
    state.sync_all_auras();
}
