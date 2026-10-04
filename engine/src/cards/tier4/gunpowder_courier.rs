//! `Gunpowder Courier` (`BG26_810`) — Tier 4 Pirate (`2/5`).
//!
//! Whenever you spend 5 Gold, give your Pirates `+3/+1` (`twice` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 426;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Gunpowder Courier", 2, 5, 4)
        .with_tribe(Tribe::Pirate)
        .on_gold_spent(on_gold_spent)
}

pub fn on_gold_spent(
    state: &mut TavernState,
    self_idx: usize,
    amount: u32,
    _: &mut CardPool,
    _: &mut Rng,
) {
    let courier = &mut state.board[self_idx];
    courier.counter += amount as i32;
    let procs = courier.counter / 5 * courier.golden_mult();
    courier.counter %= 5;
    if procs == 0 {
        return;
    }
    for u in state
        .board
        .iter_mut()
        .filter(|u| u.tribe.matches(Tribe::Pirate))
    {
        u.add_stats(3 * procs, procs);
    }
}
