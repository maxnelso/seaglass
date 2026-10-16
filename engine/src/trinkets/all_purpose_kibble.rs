//! `All-Purpose Kibble` (`BG32_MagicItem_200`) — Greater, 2 Gold, Beast.
//!
//! Whenever a friendly Beast attacks, give it +3 Attack and permanently improve this.
//!
//! `trinket_friendly_attack`: a friendly Beast that attacks gains `3 + stacks` Attack before it
//! strikes, then `stacks` goes up by 1 for the rest of the game: each improvement is +1 Attack
//! (**engine decision**). In combat or in the Tavern; the Attack is the attacking copy's.

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, Tribe, UnitId};

pub const ID: CardId = 120932;

/// The Attack before any improvement.
const BASE: i32 = 3;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_friendly_attack(friendly_attack),
    )]
}

fn friendly_attack(ctx: &mut BoardCtx<'_>, attacker_id: UnitId) {
    let Some(pos) = ctx.board.iter().position(|u| u.id == attacker_id) else {
        return;
    };
    if !ctx.board[pos].tribe.matches(Tribe::Beast) {
        return;
    }
    let Some(trinket) = super::state_mut(ctx.auras, ID) else {
        return;
    };
    let attack = BASE + trinket.stacks;
    trinket.stacks += 1;
    ctx.buff_unit(pos, attack, 0, "All-Purpose Kibble");
}
