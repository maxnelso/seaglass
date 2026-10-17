//! `Wolfhead Flail` (`BG36_MagicItem_212`) — Greater, 4 Gold, Beast.
//!
//! At the end of your turn, trigger your two left-most Deathrattles.
//!
//! `trinket_end_turn`: the two left-most minions on the board with a Deathrattle (their own or one
//! an effect gave them) each trigger their Deathrattles, left to right, in the Tavern
//! (`cards::trigger_tavern_deathrattle`: summons to the minion's right, repeated by `Titus
//! Rivendare`). "Two Deathrattles" counts minions, not Deathrattles, each
//! triggering all of its own **(confirmed)**.

use crate::cards::{self, CardHooks};
use crate::model::{CardId, UnitId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133400;

/// Minions whose Deathrattles trigger.
const MINIONS: usize = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_end_turn(|s, n, p, r| super::repeat_end_turn(s, n, p, r, end_turn)),
    )]
}

fn end_turn(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let picked: Vec<UnitId> = state
        .board
        .iter()
        .filter(|u| cards::is_deathrattle_minion(u.card_id) || !u.extra_deathrattles.is_empty())
        .take(MINIONS)
        .map(|u| u.id)
        .collect();
    for id in picked {
        if let Some(pos) = state.board.iter().position(|u| u.id == id) {
            cards::trigger_tavern_deathrattle(state, pos, pool, rng);
        }
    }
}
