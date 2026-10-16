//! `Protective Ring` (`BG35_MagicItem_711`) — Greater, 2 Gold, Pirate.
//!
//! Start of Combat: Give 4 random friendly Pirates Divine Shield.
//!
//! Step 1c (`docs/trinkets.md` §7.2): 4 different friendly Pirates (all of them if fewer) gain
//! Divine Shield, each drawn with one `rng.below(k)` among those left (**engine decision**: a
//! Pirate that already has one can be drawn).

use crate::cards::{CardHooks, HeroCombatCtx};
use crate::model::{CardId, Keyword, Tribe};

pub const ID: CardId = 130893;

/// Pirates given Divine Shield.
const PIRATES: usize = 4;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_start_of_combat(start_of_combat),
    )]
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let mut pirates: Vec<usize> = (0..ctx.own.board.len())
        .filter(|&i| ctx.own.board[i].tribe.matches(Tribe::Pirate))
        .collect();
    for _ in 0..PIRATES {
        if pirates.is_empty() {
            break;
        }
        let i = pirates.remove(ctx.own.rng.below(pirates.len()));
        ctx.own.board[i].apply_keyword(Keyword::DivineShield, false);
    }
}
