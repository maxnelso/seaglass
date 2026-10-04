//! `Devilish Distractor` (`BG36_762`) — Tier 5 Demon (`4/7`).
//!
//! Whenever you cast a spell on this, give minions in the Tavern `+1/+2` (`+2/+4` if Golden) this game.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 510;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Devilish Distractor", 4, 7, 5).with_tribe(Tribe::Demon)
}

pub fn after_cast_targeted_spell(state: &mut TavernState, target_pos: usize) {
    if target_pos >= state.board.len() || state.board[target_pos].card_id != ID {
        return;
    }
    let mult = if state.board[target_pos].is_golden { 2 } else { 1 };
    let d_atk = mult;
    let d_hp = 2 * mult;
    state.auras.tavern_all_atk += d_atk;
    state.auras.tavern_all_hp += d_hp;
    for s in &mut state.shop {
        if !s.is_spell {
            s.add_stats(d_atk, d_hp);
        }
    }
}
