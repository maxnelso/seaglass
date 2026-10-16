//! `Aggem Sticker` (`BG32_MagicItem_284`) — Greater, 2 Gold, Quilboar.
//!
//! At the end of your turn, this plays 7 Blood Gems on a friendly minion of each type.
//!
//! `trinket_end_turn`: "a friendly minion of each type" is the Menagerie rule (confirmed): the
//! best assignment of board minions to types, each minion at most once, at random among the best
//! (`types::select_menagerie_targets`, as `Misplaced Tea Set`'s). Each chosen minion has 7 Blood
//! Gems played on it (played, not cast).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};
use crate::types;

pub const ID: CardId = 121132;

/// Blood Gems per minion.
const GEMS: u32 = 7;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_end_turn(|s, n, p, r| super::repeat_end_turn(s, n, p, r, end_turn)),
    )]
}

fn end_turn(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    let auras = state.auras.clone();
    for pos in types::select_menagerie_targets(&state.board, rng) {
        state.board[pos].play_blood_gems(GEMS, &auras);
    }
}
