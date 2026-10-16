//! `Ironforge Anvil` (`BG30_MagicItem_403`) — Greater, 1 Gold.
//!
//! Start of Combat: Triple the stats of your minions with no type.
//!
//! Step 1c (`docs/trinkets.md` §7.2): each friendly minion without a type, left to right, gains
//! twice its Attack and Health.

use crate::cards::{CardHooks, HeroCombatCtx};
use crate::model::{CardId, Tribe};

pub const ID: CardId = 111488;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_start_of_combat(start_of_combat),
    )]
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    for i in 0..ctx.own.board.len() {
        let unit = &ctx.own.board[i];
        if unit.tribe == Tribe::None && !unit.is_deity {
            let (attack, health) = (unit.attack.max(0), unit.health.max(0));
            ctx.own
                .buff_unit(i, 2 * attack, 2 * health, "Ironforge Anvil");
        }
    }
}
