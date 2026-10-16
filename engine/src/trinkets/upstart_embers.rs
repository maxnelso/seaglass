//! `Upstart Embers` (`BG35_MagicItem_862`) — Greater, 6 Gold, Elemental.
//!
//! After you Refresh, double the stats of the highest- Health minion in the Tavern.
//!
//! `trinket_after_player_refresh`: after every Refresh the player does (not the turn start's new
//! shop: confirmed, `docs/tavern.md` §4.1), the minion of the highest Health in the Tavern gains
//! its Attack and Health; ties are broken at random (one `rng.below(k)` among the tied: confirmed).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130874;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_after_player_refresh(after_refresh),
    )]
}

fn after_refresh(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    let Some(best) = state
        .shop
        .iter()
        .filter(|u| !u.is_spell)
        .map(|u| u.health)
        .max()
    else {
        return;
    };
    let tied: Vec<usize> = (0..state.shop.len())
        .filter(|&i| !state.shop[i].is_spell && state.shop[i].health == best)
        .collect();
    let unit = &mut state.shop[super::pick_tied(&tied, rng)];
    let (attack, health) = (unit.attack, unit.health);
    unit.add_stats(attack, health);
}
