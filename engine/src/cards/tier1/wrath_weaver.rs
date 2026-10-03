//! `Wrath Weaver` (`BGS_004`) — Tier 1 Demon (`1/3`).
//! After you play a Demon, deal 1 damage to your hero and gain `+2/+2` (twice if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 105;
pub const NAME: &str = "Wrath Weaver";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 3, 1).with_tribe(Tribe::Demon)
}

pub fn after_play_minion(state: &mut TavernState, played_tribe: Tribe, played_board_pos: usize) {
    if !played_tribe.matches(Tribe::Demon) {
        return;
    }

    let weaver_indices: Vec<(usize, bool)> = state
        .board
        .iter()
        .enumerate()
        .filter(|(i, u)| *i != played_board_pos && u.card_id == ID)
        .map(|(i, u)| (i, u.is_golden))
        .collect();

    for (idx, is_golden) in weaver_indices {
        let triggers = if is_golden { 2 } else { 1 };
        for _ in 0..triggers {
            state.deal_hero_damage(1);
            state.board[idx].add_stats(2, 2);
        }
    }
}
