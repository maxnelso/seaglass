//! `Funeral Wreath` (`BG36_MagicItem_217`) — Greater, 3 Gold, Undead.
//!
//! After a friendly minion is Reborn, get a plain copy of it. (3 times per combat.)
//!
//! `trinket_after_reborn`: after a friendly minion comes back with Reborn (`cards::reborn`, in
//! combat or in the Tavern), a plain copy of it (`cards::instantiate_plain_copy`) to the hand (the
//! combat hand in combat); 3 times per combat (`armed`, which starts again every combat).

use crate::cards::{self, BoardCtx, CardHooks};
use crate::model::{CardId, TrinketState};

pub const ID: CardId = 133405;

/// Copies per combat.
const TIMES: u32 = 3;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_combat_start(combat_start)
            .on_trinket_after_reborn(after_reborn),
    )]
}

fn combat_start(trinket: &mut TrinketState) {
    trinket.armed = 0;
}

fn after_reborn(ctx: &mut BoardCtx<'_>, pos: usize) {
    let Some(trinket) = super::state_mut(ctx.auras, ID) else {
        return;
    };
    if trinket.armed >= TIMES {
        return;
    }
    trinket.armed += 1;
    let copy = cards::instantiate_plain_copy(&ctx.board[pos]);
    ctx.add_to_hand(copy);
}
