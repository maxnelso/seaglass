//! `Soulkeeping Jailer` (`BG36_503`) — Tier 4 Demon (`3/5`).
//!
//! Activate (2): Your Demons each consume a random minion in the Tavern to gain its (`double` its if Golden) stats.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 455;
pub const ACTIVATE_COST: u32 = 2;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Soulkeeping Jailer", 3, 5, 4)
        .with_tribe(Tribe::Demon)
        .with_activate_cost(ACTIVATE_COST)
        .on_activate(|state, source_pos, _, pool, rng| on_activate(state, source_pos, pool, rng))
}

pub fn on_activate(
    state: &mut TavernState,
    source_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let mult = if state.board[source_pos].is_golden { 2 } else { 1 };
    let demon_indices: Vec<usize> = state
        .board
        .iter()
        .enumerate()
        .filter(|(_, u)| u.tribe.matches(Tribe::Demon))
        .map(|(i, _)| i)
        .collect();
    for d_idx in demon_indices {
        let shop_minions: Vec<usize> = state
            .shop
            .iter()
            .enumerate()
            .filter(|(_, u)| !u.is_spell)
            .map(|(i, _)| i)
            .collect();
        if shop_minions.is_empty() {
            break;
        }
        let pick = if shop_minions.len() == 1 {
            shop_minions[0]
        } else {
            shop_minions[rng.below(shop_minions.len())]
        };
        let consumed = state.shop.remove(pick);
        pool.return_unit(&consumed);
        state.board[d_idx].add_stats(consumed.attack * mult, consumed.health * mult);
    }
}
