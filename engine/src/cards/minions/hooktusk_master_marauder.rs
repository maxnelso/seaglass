//! `Hooktusk, Master Marauder` (`BG36_344`) — Tier 6 Pirate (`5/5`).
//!
//! After you Discover a card, give your Pirates `+1/+1` (`+2/+2` if Golden).
//! (Improved by Golden minions you played this game!)

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 615;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Hooktusk, Master Marauder", 5, 5, 6)
        .with_tribe(Tribe::Pirate)
        .on_discover(on_discover)
}

pub fn on_discover(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let base = 1 + state.auras.golden_minions_played as i32;
    let buff = base * state.board[self_idx].golden_mult();
    for u in &mut state.board {
        if u.tribe.matches(Tribe::Pirate) {
            u.add_stats(buff, buff);
        }
    }
}
