//! `Herald Sticker` (`BG32_MagicItem_306`) — Greater, 3 Gold, Beast.
//!
//! Start of Combat: Trigger all friendly Deathrattles.
//!
//! Step 1c (`docs/trinkets.md` §7.2): each friendly minion with a Deathrattle (its own or one an
//! effect gave it), left to right as the board was when it started, if still alive, triggers its
//! Deathrattles as `Deathstrider` does (repeated by `Titus Rivendare`), its summons going to its
//! right. Deaths and destroys resolve once all have triggered.

use crate::cards::{self, CardHooks, HeroCombatCtx, Passive};
use crate::model::{CardId, UnitId};

pub const ID: CardId = 120926;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_start_of_combat(start_of_combat),
    )]
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let ctx = &mut ctx.own;
    let ids: Vec<UnitId> = ctx
        .board
        .iter()
        .filter(|u| cards::is_deathrattle_minion(u.card_id) || !u.extra_deathrattles.is_empty())
        .map(|u| u.id)
        .collect();
    for id in ids {
        let Some(pos) = ctx.board.iter().position(|u| u.id == id && u.health > 0) else {
            continue;
        };
        let unit = ctx.board[pos].clone();
        let repeats = 1
            + cards::board_passive(ctx.board, Passive::ExtraDeathrattles)
            + super::extra_deathrattles(ctx.auras, &unit);
        ctx.cursor = pos + 1;
        for _ in 0..repeats {
            cards::on_deathrattle(&unit, ctx);
        }
    }
}
