//! `Rod of Roasting` (`TB_BaconShop_HERO_35_Buddy_t6`) — Wheel of Yogg-Saron, 5%.
//!
//! Cast 'Pyrobuff' randomly to give minions +10/+10 until one hits your bartender or hero.
//!
//! Repeatedly, a random target among the board's minions, the hero and Bob, each equally likely
//! (one `rng.below(n + 2)`, `n` the minions then; confirmed): a minion gains +10/+10 and the Rod
//! goes on; the hero or Bob ends it (confirmed) and takes nothing. The +10/+10 is not a Tavern
//! spell's: no spell bonus (engine decision; `docs/wheel_of_yogg.md` §4).

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 80438;

/// What `Pyrobuff` gives a minion.
const BUFF: i32 = 10;
/// The targets besides the minions: the hero and Bob.
const ENDS: usize = 2;

/// The outcome's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, rng: &mut Rng) {
    loop {
        let target = rng.below(state.board.len() + ENDS);
        let Some(minion) = state.board.get_mut(target) else {
            return;
        };
        minion.add_stats(BUFF, BUFF);
    }
}
