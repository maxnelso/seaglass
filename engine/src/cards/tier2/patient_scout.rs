//! `Patient Scout` (`BG24_715`) — Tier 2 Neutral (`1/1`).
//! When you sell this, **Discover** a (`two` if Golden) Tier 1 minion(s). *(Improves each turn!)*

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 221;
pub const NAME: &str = "Patient Scout";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 1, 2)
        .with_tribe(Tribe::None)
        .on_sell(on_sell)
        .on_turn_start_unit(on_start_turn)
}

/// Improves each turn: `counter` counts the improvements (see [`discover_tier`]).
pub fn on_start_turn(unit: &mut Unit) {
    if unit.counter < 5 {
        unit.counter += 1;
    }
}

/// Tier of the minions this Discovers when sold (`1..=6`; starts at 1, +1 each turn).
pub fn discover_tier(unit: &Unit) -> u32 {
    (1 + unit.counter).clamp(1, 6) as u32
}

pub fn on_sell(state: &mut TavernState, sold: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    let count = if sold.is_golden { 2 } else { 1 };
    let target_tier = discover_tier(sold);
    for _ in 0..count {
        let mut opts = Vec::new();
        for t in (1..=target_tier).rev() {
            opts = pool.draw_discover_options(t, 3, rng);
            if !opts.is_empty() {
                break;
            }
        }
        for opt in &mut opts {
            state.apply_global_unit_auras(opt);
        }
        if !opts.is_empty() {
            state.push_discover(opts);
        }
    }
}
