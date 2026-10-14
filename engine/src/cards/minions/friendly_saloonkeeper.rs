//! `Friendly Saloonkeeper` (`BGDUO_104`) — Tier 2 Neutral (`3/4`), Duos only.
//! **Battlecry:** Your teammate gets a Tavern Coin (Golden `BGDUO_104_G`, `6/8`: 2 Tavern
//! Coins).
//!
//! The Coins are `Card` gifts (`docs/duos_tavern.md` §8.4): they queue like passed cards, but are
//! not Passes.

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, TeammateGift, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 10202;
pub const NAME: &str = "Friendly Saloonkeeper";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 4, 2)
        .on_battlecry(|state, unit, _, _, _| on_battlecry(state, unit))
}

/// The teammate gets a Tavern Coin (2 if Golden).
pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    for _ in 0..unit.golden_mult() {
        state
            .auras
            .give_teammate(TeammateGift::Card(tokens::make_tavern_coin()));
    }
}
