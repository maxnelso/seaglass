//! `Flaming Enforcer` (`BG34_500`) — Tier 4 Elemental/Demon (`4/5`).
//!
//! At the end of your turn, consume the highest-Health minion in the Tavern to gain its (`double` its if Golden) stats.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 420;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Flaming Enforcer", 4, 5, 4)
        .with_tribe(Tribe::ElementalDemon)
        .on_end_of_turn(on_end_turn)
}

pub fn on_end_turn(state: &mut TavernState, self_idx: usize, pool: &mut CardPool, _: &mut Rng) {
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
        let mult = if state.board[self_idx].is_golden {
            2
        } else {
            1
        };
        state.board[self_idx].add_stats(consumed.attack * mult, consumed.health * mult);
    }
}
