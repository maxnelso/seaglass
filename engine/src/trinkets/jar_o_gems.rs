//! `Jar o' Gems` (`BG30_MagicItem_546`) — Greater, 3 Gold, Quilboar.
//!
//! After a friendly Quilboar attacks each combat, this plays a Blood Gems on all your Quilboar.
//!
//! `trinket_friendly_attack`: whenever a friendly Quilboar attacks in combat, a Blood Gem is
//! played (not cast: `tavern.md` §7.4) on every living friendly Quilboar. Attacks in the Tavern
//! don't count.

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, Tribe, UnitId};

pub const ID: CardId = 112398;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_friendly_attack(friendly_attack),
    )]
}

fn friendly_attack(ctx: &mut BoardCtx<'_>, attacker_id: UnitId) {
    if !ctx.in_combat {
        return;
    }
    let Some(attacker) = ctx.board.iter().find(|u| u.id == attacker_id) else {
        return;
    };
    if !attacker.tribe.matches(Tribe::Quilboar) {
        return;
    }
    let auras = ctx.auras.clone();
    for idx in 0..ctx.board.len() {
        if ctx.board[idx].health > 0 && ctx.board[idx].tribe.matches(Tribe::Quilboar) {
            crate::cards::play_blood_gems_on_board_unit(ctx.board, idx, 1, &auras, ctx.in_combat);
        }
    }
}
