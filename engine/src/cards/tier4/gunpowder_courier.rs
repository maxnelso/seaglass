//! `Gunpowder Courier` (`BG26_810`) — Tier 4 Pirate (`2/5`).
//!
//! Whenever you spend 5 Gold, give your Pirates `+3/+1` (`twice` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 426;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Gunpowder Courier", 2, 5, 4).with_tribe(Tribe::Pirate)
}

pub fn on_gold_spent(state: &mut TavernState, amount: u32) {
    let mut total_procs = 0i32;
    for u in &mut state.board {
        if u.card_id == ID {
            u.gunpowder_gold_progress += amount;
            let triggers = u.gunpowder_gold_progress / 5;
            u.gunpowder_gold_progress %= 5;
            let mult = if u.is_golden { 2 } else { 1 };
            total_procs += (triggers as i32) * mult;
        }
    }
    if total_procs > 0 {
        for u in &mut state.board {
            if u.tribe.matches(Tribe::Pirate) {
                u.add_stats(3 * total_procs, total_procs);
            }
        }
    }
}
