//! `Faerie Dragon Scale` (`BG32_MagicItem_363`) — Greater, 4 Gold, Dragon.
//!
//! Whenever a friendly Dragon attacks, give it Divine Shield. (3 times per combat.)
//!
//! `trinket_friendly_attack`: the first 3 attacks of friendly Dragons in a combat (`armed` counts
//! them; it starts again each turn) give the attacker Divine Shield before it strikes. A Dragon
//! that still has one uses a charge too (**engine decision**). Attacks in the Tavern count as well.

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, Keyword, Tribe, UnitId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 120870;

/// Shields per combat.
const CHARGES: u32 = 3;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_friendly_attack(friendly_attack),
    )]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_combat(state, ID);
}

fn friendly_attack(ctx: &mut BoardCtx<'_>, attacker_id: UnitId) {
    let Some(pos) = ctx.board.iter().position(|u| u.id == attacker_id) else {
        return;
    };
    if !ctx.board[pos].tribe.matches(Tribe::Dragon) {
        return;
    }
    let Some(trinket) = super::state_mut(ctx.auras, ID).filter(|t| t.armed < CHARGES) else {
        return;
    };
    trinket.armed += 1;
    ctx.board[pos].apply_keyword(Keyword::DivineShield, false);
}
