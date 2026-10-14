//! `Gathering Stormer` (`BGDUO31_201`) — Tier 2 Elemental (`5/1`), Duos only.
//! When you sell this, your teammate gains `1` Gold. *(Improves each turn!)* (Golden
//! `BGDUO31_201_G`, `10/2`: `2` Gold.)
//!
//! It improves at the start of each turn it starts on the board, without a cap (`counter`, as
//! `Patient Scout`); sold, the teammate gains `1 + counter` Gold, twice that if Golden, through an
//! `Effect` gift (`docs/duos_tavern.md` §8.4).

use crate::cards::CardTemplate;
use crate::model::{CardId, TeammateGift, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10203;
pub const NAME: &str = "Gathering Stormer";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 5, 1, 2)
        .with_tribe(Tribe::Elemental)
        .on_turn_start_unit(|stormer| stormer.counter += 1)
        .on_sell(on_sell)
        .on_teammate_gift(teammate_gift)
}

/// The Gold its sale gives the teammate: `1 + counter`, twice that if Golden.
pub fn gold(stormer: &Unit) -> i32 {
    (1 + stormer.counter) * stormer.golden_mult()
}

/// Sold: the teammate gains the Gold.
pub fn on_sell(state: &mut TavernState, sold: &Unit, _: &mut CardPool, _: &mut Rng) {
    state.auras.give_teammate(TeammateGift::Effect {
        card_id: ID,
        amount: gold(sold),
        team: false,
    });
}

/// The teammate's Tavern: gain `amount` Gold.
pub fn teammate_gift(state: &mut TavernState, amount: i32, _: &mut Rng) -> Vec<Unit> {
    state.gold += amount.max(0) as u32;
    Vec::new()
}
