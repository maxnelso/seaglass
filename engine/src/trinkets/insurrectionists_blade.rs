//! `Insurrectionist's Blade` (`BG36_MagicItem_214`) — Greater, 2 Gold, Dragon, Quilboar.
//!
//! At the end of your turn, trigger all friendly Rallies.
//!
//! `trinket_end_turn`: each Rally minion on the board, left to right as the board was (if still
//! there), triggers its Rally as `Sky-hatch Runaway` makes it (`cards::trigger_tavern_rally`).

use crate::cards::{self, CardHooks};
use crate::model::{CardId, UnitId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133402;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_end_turn(|s, n, p, r| super::repeat_end_turn(s, n, p, r, end_turn)),
    )]
}

fn end_turn(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let rallies: Vec<UnitId> = state
        .board
        .iter()
        .filter(|u| cards::is_rally_minion(u.card_id))
        .map(|u| u.id)
        .collect();
    for id in rallies {
        if let Some(pos) = state.board.iter().position(|u| u.id == id) {
            cards::trigger_tavern_rally(state, pos, pool, rng);
        }
    }
}
