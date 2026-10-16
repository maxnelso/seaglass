//! `Booty Bay Brew` (`BG30_MagicItem_924`) — Lesser, 2 Gold, Pirate.
//!
//! Whenever you spend Gold, give two friendly Pirates +3/+3.
//!
//! `trinket_gold_spent`: each spending (a buy, a Refresh, an Upgrade, ...; whatever the amount)
//! gives 2 different random Pirates on the board +3/+3 (`trinkets::buff_random_pirates`: every
//! Pirate when there are 2 or fewer; **engine decision**).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 110993;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_gold_spent(gold_spent))]
}

fn gold_spent(state: &mut TavernState, _: u32, _: &mut CardPool, rng: &mut Rng) {
    super::buff_random_pirates(state, 2, 3, 3, rng);
}
