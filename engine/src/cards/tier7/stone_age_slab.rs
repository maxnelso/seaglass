//! `Stone Age Slab` (`BG34_950`) — Tier 7 Elemental (`10/10`).
//!
//! After you buy a minion, give it `+20/+20` and double (`triple` if Golden) its stats.
//! (Once per turn.)

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 711;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Stone Age Slab", 10, 10, 7)
        .with_tribe(Tribe::Elemental)
        .on_reset_turn_charges(|u| u.slab_charges_left = 1)
}

pub fn on_buy_minion(state: &mut TavernState, bought: &mut Unit) {
    for u in &mut state.board {
        if u.card_id == ID && u.slab_charges_left > 0 {
            u.slab_charges_left -= 1;
            bought.add_stats(20, 20);
            let mult = if u.is_golden { 3 } else { 2 };
            let extra_atk = bought.attack.max(0) * (mult - 1);
            let extra_hp = bought.health.max(0) * (mult - 1);
            bought.add_stats(extra_atk, extra_hp);
        }
    }
}
