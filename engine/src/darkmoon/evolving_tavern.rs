//! `Evolving Tavern` (`BGS_Treasures_006`) — Tier 2 Darkmoon Prize.
//!
//! Replace all cards in the Tavern with ones of a Tier higher.
//!
//! Each card in the Tavern, left to right, is replaced in its place (engine decision): a minion of
//! Tier `t` goes back to the pool, then a minion of exactly Tier `min(t + 1, 7)` is drawn from it
//! (the highest lower Tier with copies left if none: `CardPool::draw_discover_options`), with the
//! Tavern's buffs; a spell of Tier `t` becomes a Tavern spell of exactly Tier `min(t + 1, 7)` the
//! shop can offer (one draw). Not a Refresh; the new cards aren't frozen. A minion the pool can't
//! replace stays.

use crate::cards::spells::{self, spell};
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 65238;

/// The highest Tier a card can become.
const MAX_TIER: u32 = 7;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    for i in 0..state.shop.len() {
        let old = state.shop[i].clone();
        let tier = (old.tavern_tier + 1).min(MAX_TIER);
        if old.is_spell {
            if let Some(new) = spells::draw_discover_tavern_spells_exact_tier(tier, 1, rng).pop() {
                state.shop[i] = new;
            }
            continue;
        }
        pool.return_unit(&old);
        match pool.draw_discover_options(tier, 1, rng).pop() {
            Some(mut new) => {
                state.apply_shop_auras(&mut new);
                state.shop[i] = new;
            }
            None => pool.take_unit(&old),
        }
    }
}
