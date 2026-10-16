//! `Staff of the Scourge` (`BG30_MagicItem_437`) — Lesser, 3 Gold, Undead.
//!
//! Avenge (5): Give a random friendly Undead Reborn.
//!
//! `trinket_friendly_death`: every 5th friendly minion that dies in a combat (the count starts
//! again each turn) gives a random living friendly Undead Reborn (one `rng.below(k)`; one that
//! already has Reborn can be drawn: **engine decision**). Nothing without one.

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, Keyword, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 112080;

/// Friendly deaths per Avenge.
const AVENGE: i32 = 5;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_friendly_death(avenge),
    )]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_combat(state, ID);
}

fn avenge(ctx: &mut BoardCtx<'_>, _: &Unit) {
    if !super::avenge(ctx, ID, AVENGE) {
        return;
    }
    let undead: Vec<usize> = (0..ctx.board.len())
        .filter(|&i| ctx.board[i].health > 0 && ctx.board[i].tribe.matches(Tribe::Undead))
        .collect();
    if undead.is_empty() {
        return;
    }
    let pick = undead[ctx.rng.below(undead.len())];
    ctx.board[pick].apply_keyword(Keyword::Reborn, false);
}
