//! `Dalaran Cheese Wheel` (`BG30_MagicItem_879t`) — Greater, 0 Gold, Demon.
//!
//! Minions in the Tavern have +2/+2. Improves every 4 times you Refresh. (4 left!)
//!
//! Picked: the Tavern's bonus (`auras.tavern_all_atk` / `tavern_all_hp`, as `Staff of
//! Enrichment`'s) goes up by +2/+2, the minions in the Tavern now too (`trinkets::buff_tavern_minions`).
//! `trinket_after_player_refresh`: every 4th Refresh the player does (`counter`, across turns; not
//! the turn start's new shop: confirmed, `docs/tavern.md` §4.1) improves it by another +2/+2
//! (**engine decision**), after the Refresh's new minions got the old bonus, so they gain the new
//! +2/+2 too.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 115236;

/// Refreshes per improvement.
const REFRESHES: i32 = 4;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(improve)
            .on_trinket_after_player_refresh(after_refresh),
    )]
}

fn improve(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::buff_tavern_minions(state, 2, 2);
}

fn after_refresh(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if super::count_to(&mut state.auras, ID, REFRESHES) {
        improve(state, pool, rng);
    }
}
