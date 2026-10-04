//! `Imposing Percussionist` (`BG26_525`) — Tier 4 Demon (`4/4`).
//!
//! Battlecry: Discover a (`2` if Golden) Demon(s). Deal damage to your hero equal to its Tier.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 435;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Imposing Percussionist", 4, 4, 4).with_tribe(Tribe::Demon)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let mut opts = pool.draw_discover_by_tribe(Tribe::Demon, state.tavern_tier, 3, rng);
        for opt in &mut opts {
            state.apply_global_unit_auras(opt);
            opt.discover_deals_tier_damage = true;
        }
        if !opts.is_empty() {
            state.push_discover(opts);
        }
    }
}
