//! `Training Certificate` (`BG30_MagicItem_962`) — Lesser, 3 Gold, Dragon.
//!
//! Start of Combat: Double the stats of your two lowest-Attack minions.
//!
//! Step 1c (`docs/trinkets.md` §7.2): the two minions with the lowest Attack gain their Attack and
//! Health, the left one first. Each is picked in turn, the lowest Attack left; ties are broken at
//! random (one `rng.below(k)` among the tied, none for a single one: confirmed).

use crate::cards::{CardHooks, HeroCombatCtx};
use crate::model::CardId;

pub const ID: CardId = 111417;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_start_of_combat(start_of_combat),
    )]
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let mut left: Vec<usize> = (0..ctx.own.board.len()).collect();
    let mut lowest = Vec::new();
    while lowest.len() < 2 {
        let Some(min) = left.iter().map(|&i| ctx.own.board[i].attack).min() else {
            break;
        };
        let tied: Vec<usize> = left
            .iter()
            .copied()
            .filter(|&i| ctx.own.board[i].attack == min)
            .collect();
        let pick = super::pick_tied(&tied, ctx.own.rng);
        left.retain(|&i| i != pick);
        lowest.push(pick);
    }
    lowest.sort_unstable();
    for i in lowest {
        let (attack, health) = (ctx.own.board[i].attack, ctx.own.board[i].health);
        ctx.own
            .buff_unit(i, attack.max(0), health.max(0), "Training Certificate");
    }
}
