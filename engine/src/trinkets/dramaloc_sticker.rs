//! `Dramaloc Sticker` (`BG35_MagicItem_754`) — Greater, 6 Gold, Murloc.
//!
//! Start of Combat: Give your Murlocs the Attack of the highest-Attack minion in your hand.
//!
//! Step 1c (`docs/trinkets.md` §7.2): each friendly Murloc, left to right, gains Attack equal to
//! the highest Attack of a minion in the hand (**engine decision**: it gains that much, rather
//! than its Attack becoming it). Without a minion in the hand, nothing happens.

use crate::cards::{CardHooks, HeroCombatCtx};
use crate::model::{CardId, Tribe};

pub const ID: CardId = 131307;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_start_of_combat(start_of_combat),
    )]
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let Some(attack) = ctx
        .own
        .hand
        .iter()
        .filter(|u| !u.is_spell)
        .map(|u| u.attack)
        .max()
        .filter(|&a| a > 0)
    else {
        return;
    };
    for i in 0..ctx.own.board.len() {
        if ctx.own.board[i].tribe.matches(Tribe::Murloc) {
            ctx.own.buff_unit(i, attack, 0, "Dramaloc Sticker");
        }
    }
}
