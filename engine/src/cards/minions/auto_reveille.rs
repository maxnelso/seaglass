//! `Auto Reveille` (`BG36_367`) — Tier 6 Mech (`4/8`).
//!
//! After you buy 3 cards, Magnetize a random Volumizer to this and get a copy (`2` copies if Golden) of it.

use crate::cards::{self, minions, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 601;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Auto Reveille", 4, 8, 6)
        .with_tribe(Tribe::Mech)
        .on_after_buy(after_buy)
}

pub fn after_buy(
    state: &mut TavernState,
    self_idx: usize,
    _: &Unit,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let reveille = &mut state.board[self_idx];
    reveille.counter += 1;
    let procs = reveille.counter / 3;
    reveille.counter %= 3;
    let copies = if reveille.is_golden { 2 } else { 1 };
    for _ in 0..procs {
        let mut vol = minions::conveyor_construct::draw_random_volumizer(rng);
        state.apply_global_unit_auras(&mut vol);
        let copy = vol.clone();
        cards::magnetize(state, &mut vol, self_idx, pool, rng);
        for _ in 0..copies {
            state.add_to_hand(copy.clone());
        }
    }
}
