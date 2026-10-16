//! `Upstart Embers` (`BG35_MagicItem_862`) — Greater, 6 Gold, Elemental.
//!
//! After you Refresh, double the stats of the highest- Health minion in the Tavern.
//!
//! `trinket_after_refresh`: every Refresh (the turn start's new shop too, as for
//! `hero_after_refresh`: **engine decision**), the minion of the highest Health in the Tavern
//! (ties to the left-most: **engine decision**) gains its Attack and Health.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130874;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_refresh(after_refresh))]
}

fn after_refresh(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let mut best: Option<usize> = None;
    for (i, unit) in state.shop.iter().enumerate() {
        if !unit.is_spell && best.is_none_or(|b| unit.health > state.shop[b].health) {
            best = Some(i);
        }
    }
    if let Some(i) = best {
        let unit = &mut state.shop[i];
        let (attack, health) = (unit.attack, unit.health);
        unit.add_stats(attack, health);
    }
}
