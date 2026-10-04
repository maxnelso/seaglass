//! `Polarizing Beatboxer` (`BG26_149`) — Tier 7 Mech (`5/10`).
//!
//! Whenever you Magnetize to a different minion, it also Magnetizes to this (`twice` if Golden).

use crate::cards::{self, tier2, CardTemplate};
use crate::model::{CardId, Keyword, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 707;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Polarizing Beatboxer", 5, 10, 7).with_tribe(Tribe::Mech)
}

pub fn after_magnetize_to_minion(state: &mut TavernState, mag_card: &Unit, target_pos: usize) {
    let beatboxers: Vec<(usize, u32)> = state
        .board
        .iter()
        .enumerate()
        .filter(|&(i, u)| i != target_pos && u.card_id == ID)
        .map(|(i, u)| (i, if u.is_golden { 2 } else { 1 }))
        .collect();

    if beatboxers.is_empty() {
        return;
    }

    for (idx, base_mult) in beatboxers {
        if idx >= state.board.len() {
            continue;
        }
        let extra = 1 + state.board[idx].extra_magnetize_this_turn;
        state.board[idx].extra_magnetize_this_turn = 0;
        let total_repeats = base_mult * extra;
        for _ in 0..total_repeats {
            {
                let target = &mut state.board[idx];
                target.add_stats(mag_card.attack, mag_card.health);
                target.taunt |= mag_card.taunt;
                if mag_card.divine_shield {
                    target.apply_keyword(Keyword::DivineShield, false);
                }
                target.windfury |= mag_card.windfury;
                target.reborn |= mag_card.reborn;
                target.venomous |= mag_card.venomous;
                target.stealth |= mag_card.stealth;
                target.magnetic |= mag_card.magnetic;
                cards::on_magnetize_transfer(mag_card, target);
            }
            tier2::mechagnome_interpreter::after_play_or_magnetize_mech(
                state,
                mag_card.tribe,
                idx,
                true,
            );
        }
    }
    cards::sync_board_spell_auras(&state.board, &mut state.auras);
}
