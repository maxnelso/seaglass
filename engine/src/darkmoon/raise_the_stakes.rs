//! `Raise the Stakes` (`BGS_Treasures_016`) — Tier 4 Darkmoon Prize.
//!
//! Make a friendly minion Golden and return it to your hand.
//!
//! A friendly non-Golden minion (engine decision: a Golden one is no target) becomes Golden
//! (`make_golden`: a Golden an effect made, which still gives a Triple Reward when played), then
//! leaves the board for the hand, as `Repeat Customer`'s does (it stays with a full hand).

use crate::cards::spells::targeted;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 65260;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        targeted(cast).on_spell_target_filter(|unit| !unit.is_golden),
    )]
}

fn cast(state: &mut TavernState, _: &Unit, pos: usize, _: &mut CardPool, _: &mut Rng) {
    if let Some(unit) = state.board.get_mut(pos) {
        unit.make_golden();
        super::return_to_hand(state, pos);
    }
}
