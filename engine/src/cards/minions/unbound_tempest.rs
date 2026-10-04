//! `Unbound Tempest` (`BG36_352`) — Tier 6 Elemental (`3/12`).
//!
//! After you play 4 Elementals, gain (`double` if Golden) the stats of the highest-Health minion in the Tavern.

use crate::cards::{CardTemplate, Played};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 628;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Unbound Tempest", 3, 12, 6)
        .with_tribe(Tribe::Elemental)
        .on_after_friendly_play(after_friendly_play)
}

pub fn after_friendly_play(
    state: &mut TavernState,
    self_idx: usize,
    played: &Played,
    _: &mut CardPool,
    _: &mut Rng,
) {
    if played.magnetized || !played.tribe.matches(Tribe::Elemental) {
        return;
    }
    let best_shop = state
        .shop
        .iter()
        .filter(|u| !u.is_spell)
        .max_by_key(|u| (u.health, u.attack))
        .map(|u| (u.attack.max(0), u.health.max(0)));
    let tempest = &mut state.board[self_idx];
    tempest.counter += 1;
    while tempest.counter >= 4 {
        tempest.counter -= 4;
        if let Some((atk, hp)) = best_shop {
            let mult = if tempest.is_golden { 2 } else { 1 };
            tempest.add_stats(atk * mult, hp * mult);
        }
    }
}
