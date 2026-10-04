//! `Flaming Enforcer` (`BG34_500`) — Tier 4 Elemental/Demon (`4/5`).
//!
//! At the end of your turn, consume the highest-Health minion in the Tavern to gain its (`double` its if Golden) stats.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 420;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Flaming Enforcer", 4, 5, 4).with_tribe(Tribe::ElementalDemon)
}

pub fn on_end_turn(state: &mut TavernState, pool: &mut CardPool) {
    let indices: Vec<(usize, bool)> = state
        .board
        .iter()
        .enumerate()
        .filter(|(_, u)| u.card_id == ID)
        .map(|(i, u)| (i, u.is_golden))
        .collect();
    for (b_idx, is_golden) in indices {
        let best_shop_idx = state
            .shop
            .iter()
            .enumerate()
            .filter(|(_, u)| !u.is_spell)
            .max_by_key(|(_, u)| (u.health, u.attack))
            .map(|(i, _)| i);
        if let Some(s_idx) = best_shop_idx {
            let consumed = state.shop.remove(s_idx);
            pool.return_unit(&consumed);
            let mult = if is_golden { 2 } else { 1 };
            state.board[b_idx].add_stats(consumed.attack * mult, consumed.health * mult);
        }
    }
}
