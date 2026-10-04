//! `Rodeo Performer` (`BG28_550`) — Tier 5 Neutral (`3/4`).
//!
//! Battlecry: Discover a (`2` if Golden) Tavern spell(s).

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 542;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Rodeo Performer", 3, 4, 5)
        .on_battlecry(|state, unit, _, _, rng| on_battlecry(state, unit, rng))
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, rng: &mut Rng) {
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let opts = spells::draw_discover_tavern_spells(state.tavern_tier, 3, rng);
        if !opts.is_empty() {
            state.push_discover(opts);
        }
    }
}
