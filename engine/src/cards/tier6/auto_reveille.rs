//! `Auto Reveille` (`BG36_367`) — Tier 6 Mech (`4/8`).
//!
//! After you buy 3 cards, Magnetize a random Volumizer to this and get a copy (`2` copies if Golden) of it.

use crate::cards::{self, tier2, tier4, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 601;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Auto Reveille", 4, 8, 6).with_tribe(Tribe::Mech)
}

pub fn after_buy_card(state: &mut TavernState, rng: &mut Rng) {
    let mut procs: Vec<(usize, bool)> = Vec::new();
    for (idx, u) in state.board.iter_mut().enumerate() {
        if u.card_id == ID {
            u.auto_reveille_buys += 1;
            while u.auto_reveille_buys >= 3 {
                u.auto_reveille_buys -= 3;
                procs.push((idx, u.is_golden));
            }
        }
    }
    for (board_idx, is_golden) in procs {
        if board_idx >= state.board.len() {
            continue;
        }
        let mut vol = tier4::conveyor_construct::draw_random_volumizer(rng);
        state.apply_global_unit_auras(&mut vol);
        let copy_template = vol.clone();
        cards::on_first_play_or_magnetize(state, &mut vol);
        let target = &mut state.board[board_idx];
        target.add_stats(vol.attack, vol.health);
        target.magnetic |= vol.magnetic;
        cards::on_magnetize_transfer(&vol, target);
        tier2::mechagnome_interpreter::after_play_or_magnetize_mech(
            state,
            Tribe::Mech,
            board_idx,
            true,
        );
        let copies = if is_golden { 2 } else { 1 };
        for _ in 0..copies {
            state.add_to_hand(copy_template.clone());
        }
    }
}
