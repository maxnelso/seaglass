//! `Faerie Dragon Scale` (`BG32_MagicItem_363`) — Greater, 4 Gold, Dragon.
//!
//! Whenever a friendly Dragon attacks, give it Divine Shield. (3 times per combat.)
//!
//! `trinket_friendly_attack`: an attacking friendly Dragon without Divine Shield gains one before it
//! strikes, 3 times per combat (`armed` counts them; it starts again each combat: confirmed). A
//! Dragon that still has one keeps it and uses no charge (confirmed). Attacks in the Tavern count
//! as well.

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, Keyword, Tribe, UnitId};

pub const ID: CardId = 120870;

/// Shields per combat.
const CHARGES: u32 = 3;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_combat_start(super::reset_per_combat)
            .on_trinket_friendly_attack(friendly_attack),
    )]
}

fn friendly_attack(ctx: &mut BoardCtx<'_>, attacker_id: UnitId) {
    let Some(pos) = ctx.board.iter().position(|u| u.id == attacker_id) else {
        return;
    };
    if !ctx.board[pos].tribe.matches(Tribe::Dragon) || ctx.board[pos].divine_shield {
        return;
    }
    let Some(trinket) = super::state_mut(ctx.auras, ID).filter(|t| t.armed < CHARGES) else {
        return;
    };
    trinket.armed += 1;
    ctx.board[pos].apply_keyword(Keyword::DivineShield, false);
}
