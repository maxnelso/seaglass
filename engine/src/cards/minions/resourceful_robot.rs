//! `Resourceful Robot` (`BG36_366`) — Tier 5 Mech (`4/8`).
//!
//! At the end of your turn, Magnetize a (`2` if Golden) random Volumizer(s) to this. Get a copy of it.

use crate::cards::{self, minions, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 541;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Resourceful Robot", 4, 8, 5)
        .with_tribe(Tribe::Mech)
        .on_end_of_turn(on_end_turn)
}

pub fn on_end_turn(state: &mut TavernState, self_idx: usize, pool: &mut CardPool, rng: &mut Rng) {
    let vol_templates = [
        minions::blue_volumizer::template(),
        minions::green_volumizer::template(),
        minions::red_volumizer::template(),
    ];
    let count = if state.board[self_idx].is_golden {
        2
    } else {
        1
    };
    for _ in 0..count {
        let pick = rng.below(vol_templates.len());
        let tpl = &vol_templates[pick];
        let mut vol = tpl.instantiate();
        state.apply_global_unit_auras(&mut vol);
        cards::magnetize(state, &mut vol, self_idx, pool, rng);

        let mut copy = tpl.instantiate();
        state.apply_global_unit_auras(&mut copy);
        state.add_to_hand(copy);
    }
}
