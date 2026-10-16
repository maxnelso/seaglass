//! `Lightfeather Sticker` (`BG36_MagicItem_213`) — Greater, 0 Gold, Beast.
//!
//! Start of Combat: Give your Rally minions Divine Shield.
//!
//! Step 1c (`docs/trinkets.md` §7.2): every friendly minion with a Rally gains Divine Shield.

use crate::cards::{self, CardHooks, HeroCombatCtx};
use crate::model::{CardId, Keyword};

pub const ID: CardId = 133401;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_start_of_combat(start_of_combat),
    )]
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    for unit in ctx.own.board.iter_mut() {
        if cards::is_rally_minion(unit.card_id) {
            unit.apply_keyword(Keyword::DivineShield, false);
        }
    }
}
