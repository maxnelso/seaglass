//! `Resourceful Robot` (`BG36_366`) — Tier 5 Mech (`4/8`).
//!
//! At the end of your turn, Magnetize a (`2` if Golden) random Volumizer(s) to this. Get a copy of it.

use crate::cards::{
    after_play_minion, on_first_play_or_magnetize, on_magnetize_transfer, tier2, tier7,
    CardTemplate,
};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 541;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Resourceful Robot", 4, 8, 5).with_tribe(Tribe::Mech)
}

pub fn on_end_turn(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let vol_templates = [
        tier2::blue_volumizer::template(),
        tier2::green_volumizer::template(),
        tier2::red_volumizer::template(),
    ];
    let robots: Vec<(usize, usize)> = state
        .board
        .iter()
        .enumerate()
        .filter(|(_, u)| u.card_id == ID)
        .map(|(i, u)| (i, if u.is_golden { 2 } else { 1 }))
        .collect();
    for (board_pos, count) in robots {
        if board_pos >= state.board.len() {
            continue;
        }
        for _ in 0..count {
            let pick = rng.below(vol_templates.len());
            let tpl = &vol_templates[pick];
            let mut vol = tpl.instantiate();
            state.apply_global_unit_auras(&mut vol);
            on_first_play_or_magnetize(state, &mut vol);

            let target = &mut state.board[board_pos];
            target.add_stats(vol.attack, vol.health);
            on_magnetize_transfer(&vol, target);
            after_play_minion(state, vol.card_id, Tribe::Mech, board_pos, true, pool, rng);
            tier7::polarizing_beatboxer::after_magnetize_to_minion(state, &vol, board_pos);

            let mut copy = tpl.instantiate();
            state.apply_global_unit_auras(&mut copy);
            state.add_to_hand(copy);
        }
    }
}
