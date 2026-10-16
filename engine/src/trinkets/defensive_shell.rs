//! `Defensive Shell` (`BG36_MagicItem_811`) — Lesser, 1 Gold.
//!
//! The first minion you play each turn gains Divine Shield.
//!
//! `trinket_after_play`: the first minion played from the hand each turn (`armed`, which starts
//! again each turn) gains Divine Shield once its play resolved; a Magnetic one Magnetized onto a
//! friendly minion gives it to that minion (**engine decision**).

use crate::cards::{CardHooks, Played};
use crate::model::{CardId, Keyword};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133063;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_after_play(after_play),
    )]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_turn(state, ID);
}

fn after_play(state: &mut TavernState, played: &Played, _: &mut CardPool, _: &mut Rng) {
    if !played.from_hand {
        return;
    }
    let Some(t) = super::state_mut(&mut state.auras, ID).filter(|t| t.armed == 0) else {
        return;
    };
    t.armed = 1;
    if let Some(unit) = state.board.get_mut(played.board_pos) {
        unit.apply_keyword(Keyword::DivineShield, false);
    }
}
