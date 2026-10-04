//! `Sellemental` (`BGS_115`) — Tier 2 Elemental (`3/3`).
//! When you sell this, get a (`two` if Golden) `3/3` Elemental(s) (`Water Droplet`).

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 226;
pub const NAME: &str = "Sellemental";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 2)
        .with_tribe(Tribe::Elemental)
        .on_sell(|state, sold, _, _| on_sell(state, sold))
}

pub fn on_sell(state: &mut TavernState, sold: &Unit) {
    let count = if sold.is_golden { 2 } else { 1 };
    for _ in 0..count {
        if state.hand.len() < 10 {
            let mut droplet = tokens::make_water_droplet();
            state.apply_global_unit_auras(&mut droplet);
            state.add_to_hand(droplet);
        }
    }
}
