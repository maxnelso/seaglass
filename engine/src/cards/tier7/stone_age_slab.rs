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
        .on_reset_turn_charges(|u| u.charges = 1)
        .on_minion_bought(on_minion_bought)
}

pub fn on_minion_bought(state: &mut TavernState, self_idx: usize, bought: &mut Unit) {
    let slab = &mut state.board[self_idx];
    if slab.charges == 0 {
        return;
    }
    slab.charges -= 1;
    let mult = if slab.is_golden { 3 } else { 2 };
    bought.add_stats(20, 20);
    let extra_atk = bought.attack.max(0) * (mult - 1);
    let extra_hp = bought.health.max(0) * (mult - 1);
    bought.add_stats(extra_atk, extra_hp);
}
