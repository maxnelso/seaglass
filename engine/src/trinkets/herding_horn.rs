//! `Herding Horn` (`BG36_MagicItem_200`) — Lesser, 3 Gold, Beast.
//!
//! Whenever a friendly Rally minion attacks, gain a free Refresh.
//!
//! `trinket_friendly_attack`: each attack of a friendly Rally minion (in combat, or a Tavern
//! attack) adds a free Refresh (`auras.free_refreshes`, which carries over).

use crate::cards::{self, BoardCtx, CardHooks};
use crate::model::{CardId, UnitId};

pub const ID: CardId = 133387;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_friendly_attack(attack))]
}

fn attack(ctx: &mut BoardCtx<'_>, attacker_id: UnitId) {
    let rally = ctx
        .board
        .iter()
        .find(|u| u.id == attacker_id)
        .is_some_and(|u| cards::is_rally_minion(u.card_id));
    if rally {
        ctx.auras.free_refreshes += 1;
    }
}
