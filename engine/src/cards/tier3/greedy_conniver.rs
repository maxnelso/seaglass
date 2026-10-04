//! `Greedy Conniver` (`BG36_369`) — Tier 3 Pirate (`7/7`).
//! If this is Golden when you sell it, **Discover** a Tier 7 minion.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 318;
pub const NAME: &str = "Greedy Conniver";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 7, 7, 3)
        .with_tribe(Tribe::Pirate)
        .on_sell(|state, sold, pool, rng| on_sell(state, sold, pool, rng))
}

pub fn on_sell(state: &mut TavernState, sold: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    if !sold.is_golden {
        return;
    }
    let mut opts = pool.draw_discover_options(7, 3, rng);
    for opt in &mut opts {
        state.apply_global_unit_auras(opt);
    }
    if !opts.is_empty() {
        state.push_discover(opts);
    }
}
