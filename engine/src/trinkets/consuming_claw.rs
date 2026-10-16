//! `Consuming Claw` (`BG36_MagicItem_801`) — Lesser, 2 Gold, Demon.
//!
//! Whenever a friendly Demon consumes a minion, it also gains its Bonus Keywords and +5/+5.
//!
//! `trinket_after_consume`: a Demon on the board that consumed a minion in the Tavern (any card's
//! consume, and a Demon Fodder a Refresh gives a Demon: **engine decision**) gains the consumed
//! minion's bonus keywords (`BONUS_KEYWORDS`) and +5/+5, after the stats the consume gave.

use crate::cards::CardHooks;
use crate::model::{CardId, Tribe, Unit, BONUS_KEYWORDS};
use crate::tavern::{MinionSlot, TavernState};

pub const ID: CardId = 133061;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_consume(consumed))]
}

fn consumed(state: &mut TavernState, consumer: MinionSlot, consumed: &Unit) {
    let MinionSlot::Board(pos) = consumer else {
        return;
    };
    let Some(demon) = state.board.get_mut(pos) else {
        return;
    };
    if !demon.tribe.matches(Tribe::Demon) {
        return;
    }
    for kw in BONUS_KEYWORDS {
        if consumed.has_keyword(kw) {
            demon.apply_keyword(kw, false);
        }
    }
    demon.add_stats(5, 5);
}
