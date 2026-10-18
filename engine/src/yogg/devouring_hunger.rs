//! `Devouring Hunger` (`TB_BaconShop_HERO_35_Buddy_t5`) — Wheel of Yogg-Saron, 19%.
//!
//! Consume all minions in the Tavern and give their stats to your minions. Then Refresh it.
//!
//! One random board minion (`rng.below(n)`, none for one) consumes every minion in the Tavern, left
//! to right, and gains their full stats (confirmed): each is a consume
//! (`trinkets::consume_shop_minion`: its copy returns to the pool, the consume observers see it).
//! The spells in the Tavern stay. Then a free Refresh that the player does
//! (`TavernState::refresh_shop_free`, `docs/tavern.md` §4.1). With no friendly minion, nothing is
//! consumed and the Tavern is still Refreshed (engine decision; `docs/wheel_of_yogg.md` §4).

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 80437;

/// The outcome's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    if !state.board.is_empty() {
        let all: Vec<usize> = (0..state.board.len()).collect();
        let consumer = crate::trinkets::pick_tied(&all, rng);
        let mut i = 0;
        while i < state.shop.len() && consumer < state.board.len() {
            if state.shop[i].is_spell {
                i += 1;
                continue;
            }
            crate::trinkets::consume_shop_minion(state, consumer, i, pool);
        }
    }
    state.refresh_shop_free(pool, rng);
}
