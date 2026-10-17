//! `Hammer of Twilight` (`BG36_MagicItem_403`) — Lesser, 2 Gold.
//!
//! Your minions have +1 Attack. (Improved by each card you've discarded this game!)
//!
//! `trinket_board_aura`: every minion on the board, in the Tavern and in combat (those played or
//! summoned later too), has +1 Attack × (1 + the cards discarded this game, `cards_discarded`,
//! those before the pick too); not the minions in the hand or the Tavern **(confirmed)**. An aura:
//! a minion that leaves the board loses it (`cards::sync_board_unit_auras`), and it grows as the
//! player discards (`trinkets::hammer_of_twilight`).

use crate::cards::CardHooks;
use crate::model::{CardId, PlayerAuras, TrinketState, Unit};

pub const ID: CardId = 134316;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    super::hammer_of_twilight(ID, aura)
}

fn aura(_: &TrinketState, _: &Unit, auras: &PlayerAuras) -> (i32, i32) {
    super::hammer_bonus(auras, 1, 0)
}
