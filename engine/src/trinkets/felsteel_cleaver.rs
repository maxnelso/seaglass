//! `Felsteel Cleaver` (`BG36_MagicItem_831`) — Greater, 0 Gold, Demon, Quilboar.
//!
//! After you cast a spell on a Tavern minion, a random friendly minion consumes it to gain its stats.
//!
//! `trinket_after_spell`: after a spell resolved on a minion in the Tavern (any spell, from the
//! hand or cast by an effect) that is still there, a random board minion (one draw) consumes it
//! (`trinkets::consume_shop_minion`: its copy returns to the pool; the consume observers see it).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

use super::SpellCast;

pub const ID: CardId = 133067;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_spell(after_spell))]
}

fn after_spell(state: &mut TavernState, cast: &SpellCast, pool: &mut CardPool, rng: &mut Rng) {
    let Some(MinionSlot::Shop(idx)) = cast.target else {
        return;
    };
    if cast.casts == 0 || state.board.is_empty() || state.shop.get(idx).is_none_or(|u| u.is_spell) {
        return;
    }
    let all: Vec<usize> = (0..state.board.len()).collect();
    let consumer = super::pick_tied(&all, rng);
    super::consume_shop_minion(state, consumer, idx, pool);
}
