//! `Tad` (`BG22_202`) — Tier 2 Murloc (`2/2`).
//! When you sell this, get another random Murloc (`2` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 230;
pub const NAME: &str = "Tad";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 2, 2)
        .with_tribe(Tribe::Murloc)
        .on_sell(|state, sold, pool, rng| on_sell(state, sold, pool, rng))
}

pub fn on_sell(state: &mut TavernState, sold: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    let count = if sold.is_golden { 2 } else { 1 };
    for _ in 0..count {
        if state.hand.len() >= 10 {
            break;
        }
        if let Some(mut murloc) =
            pool.draw_by_tribe(Tribe::Murloc, Some(ID), state.tavern_tier, rng)
        {
            state.apply_global_unit_auras(&mut murloc);
            state.add_to_hand(murloc);
        }
    }
}
