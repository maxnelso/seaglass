//! `Trusty Crowbar` (`BG35_MagicItem_713`) — Greater, 0 Gold, Pirate.
//!
//! Whenever you get a Pirate, give your left-most minion +16/+16.
//!
//! `trinket_card_added`: each Pirate minion card that enters the hand (bought, Discovered,
//! generated, delivered from the waiting queue; not a Pirate summoned or played: confirmed) gives
//! the left-most minion on the board +16/+16.

use crate::cards::CardHooks;
use crate::model::{CardId, Tribe, Unit};

use crate::tavern::TavernState;

pub const ID: CardId = 130894;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_card_added(card_added))]
}

fn card_added(state: &mut TavernState, card: &Unit) {
    if card.is_spell || !card.tribe.matches(Tribe::Pirate) {
        return;
    }
    if let Some(unit) = state.board.first_mut() {
        unit.add_stats(16, 16);
    }
}
