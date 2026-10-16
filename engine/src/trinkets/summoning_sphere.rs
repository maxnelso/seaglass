//! `Summoning Sphere` (`BGDUO_MagicItem_003`) — Lesser, 4 Gold, Duos only.
//!
//! Start of Combat: Summon a copy of your teammate's highest-Health minion.
//!
//! Step 1c (`docs/trinkets.md` §7.2): a copy of the minion with the most Health in the teammate's
//! warband as it entered the combat (`duos.md` §2; a tie at random, one `rng.below(k)`), with its
//! stats, keywords and state, is summoned at the right end of the board if it has room. Without a
//! teammate minion (or in Solo), nothing happens.

use crate::cards::{CardHooks, HeroCombatCtx};
use crate::heroes::HERO_SOURCE;
use crate::model::CardId;

pub const ID: CardId = 117934;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_start_of_combat(start_of_combat),
    )]
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let ctx = &mut ctx.own;
    let Some(best) = ctx.teammate_board.iter().map(|u| u.health).max() else {
        return;
    };
    let tied: Vec<usize> = (0..ctx.teammate_board.len())
        .filter(|&i| ctx.teammate_board[i].health == best)
        .collect();
    let pick = tied[ctx.rng.below(tied.len())];
    let mut copy = ctx.teammate_board[pick].clone();
    copy.marked = false;
    ctx.cursor = ctx.board.len();
    ctx.summon_as(HERO_SOURCE, copy, "Summoning Sphere");
}
