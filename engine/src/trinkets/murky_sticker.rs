//! `Murky Sticker` (`BG35_MagicItem_753`) — Greater, 0 Gold, Murloc.
//!
//! At the end of each turn, give your two left-most minions +1/+2. (Improved by your Battlecries triggered this game!)
//!
//! `trinket_battlecry_triggered`: each trigger of a friendly Battlecry in the Tavern since it was
//! picked adds one to `counter`. `trinket_end_turn`: the two left-most minions on the board gain
//! +1/+2 × (1 + `counter`): each Battlecry triggered adds the base amount again (**engine
//! decision**).

use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131306;

/// The base buff.
const BUFF: (i32, i32) = (1, 2);

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_battlecry_triggered(triggered)
            .on_trinket_end_turn(|s, n, p, r| super::repeat_end_turn(s, n, p, r, end_turn)),
    )]
}

fn triggered(state: &mut TavernState, _: &Unit) {
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        t.counter = t.counter.saturating_add(1);
    }
}

fn end_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let times = 1 + super::state(&state.auras, ID).map_or(0, |t| t.counter);
    for unit in state.board.iter_mut().take(2) {
        unit.add_stats(BUFF.0.saturating_mul(times), BUFF.1.saturating_mul(times));
    }
}
