//! `Mirror Monster` (`BGDUO_108`) — Tier 4 All (`4/4`), Duos only.
//! When you buy or Discover this, get an extra copy and Pass it (Golden `BGDUO_108_G`, `8/8`: 2
//! extra copies).
//!
//! Its `obtained` hook: it Passes copies of the card as it reached the hand (stats and Golden
//! status included), free Passes that count as the team's. The copies don't come from the pool
//! (`docs/duos_tavern.md` §8.3).

use crate::cards::CardTemplate;
use crate::model::{CardId, TeammateGift, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10404;
pub const NAME: &str = "Mirror Monster";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 4, 4)
        .with_tribe(Tribe::All)
        .on_obtained(on_obtained)
}

/// Bought or Discovered: Pass a copy of it (2 if Golden).
pub fn on_obtained(state: &mut TavernState, card: &Unit, _: &mut CardPool, _: &mut Rng) {
    for _ in 0..card.golden_mult() {
        state.auras.give_teammate(TeammateGift::Pass(card.clone()));
    }
}
