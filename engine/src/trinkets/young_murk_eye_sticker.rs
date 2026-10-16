//! `Young Murk-Eye Sticker` (`BG35_MagicItem_752`) — Greater, 4 Gold, Murloc.
//!
//! At the end of your turn, trigger your left and right- most minions' Battlecries.
//!
//! `trinket_end_turn`: the left-most minion on the board, then the right-most (once with a single
//! minion), triggers its Battlecry as `Young Murk-Eye` makes it (`cards::trigger_board_battlecry`);
//! nothing for one without a Battlecry. The right-most is taken once the first Battlecry resolved
//! (**engine decision**).

use crate::cards::{self, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131305;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_end_turn(|s, n, p, r| super::repeat_end_turn(s, n, p, r, end_turn)),
    )]
}

fn end_turn(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let minions = state.board.len();
    if minions == 0 {
        return;
    }
    cards::trigger_board_battlecry(state, 0, pool, rng);
    // The right-most as the board is once the first Battlecry resolved (**engine decision**).
    if let Some(last) = state.board.len().checked_sub(1).filter(|_| minions > 1) {
        cards::trigger_board_battlecry(state, last, pool, rng);
    }
}
