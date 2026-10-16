//! `Accord-o-Tron Portrait` (`BG35_MagicItem_742`) — Greater, 3 Gold, Mech.
//!
//! At the end of each turn, Magnetize an Accord-o-Tron to your left- and right-most Mechs.
//!
//! `trinket_end_turn`: a plain `Accord-o-Tron` is Magnetized by an effect (`cards::magnetize`, as
//! `Glambot`'s Satellites) onto the left-most Mech on the board, then another onto the right-most
//! (once with a single Mech). Nothing without a Mech.

use crate::cards::{self, CardHooks};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131140;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_end_turn(|s, n, p, r| super::repeat_end_turn(s, n, p, r, end_turn)),
    )]
}

fn end_turn(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let mechs: Vec<usize> = (0..state.board.len())
        .filter(|&i| state.board[i].tribe.matches(Tribe::Mech))
        .collect();
    for pos in super::ends(&mechs) {
        let Some(template) = cards::template(cards::minions::accord_o_tron::ID) else {
            return;
        };
        let mut accord = template.instantiate();
        if pos < state.board.len() {
            cards::magnetize(state, &mut accord, pos, pool, rng);
        }
    }
}
