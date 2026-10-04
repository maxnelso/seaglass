//! `Primalfin Lookout` (`BGS_020`) — Tier 5 Murloc (`3/2`).
//!
//! Battlecry: If you control another Murloc, Discover a (`2` if Golden) Murloc(s).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 538;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Primalfin Lookout", 3, 2, 5).with_tribe(Tribe::Murloc)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    if !state.board.iter().any(|u| u.tribe.matches(Tribe::Murloc)) {
        return;
    }
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let mut opts =
            pool.draw_discover_by_tribe(Tribe::Murloc, state.tavern_tier, 3, rng);
        for opt in &mut opts {
            state.apply_global_unit_auras(opt);
        }
        if !opts.is_empty() {
            state.push_discover(opts);
        }
    }
}
