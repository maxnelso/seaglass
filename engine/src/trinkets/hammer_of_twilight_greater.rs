//! `Hammer of Twilight (Greater)` (`BG36_MagicItem_403t`) — Greater, 3 Gold.
//!
//! Your minions have +2/+1. (Improved by each card you've discarded this game!)
//!
//! As the Lesser one, +2/+1 × (1 + the cards discarded this game) **(confirmed)**.

use crate::cards::CardHooks;
use crate::model::{CardId, PlayerAuras, TrinketState, Unit};

pub const ID: CardId = 134320;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    super::hammer_of_twilight(ID, aura)
}

fn aura(_: &TrinketState, _: &Unit, auras: &PlayerAuras) -> (i32, i32) {
    super::hammer_bonus(auras, 2, 1)
}
