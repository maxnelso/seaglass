//! `Nadina the Red` (`BGS_040`) — Tier 6 Neutral (`8/4`).
//!
//! Deathrattle: Give 3 (`6` if Golden) friendly Dragons Divine Shield.

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, Keyword, Tribe, Unit};

pub const ID: CardId = 617;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Nadina the Red", 8, 4, 6)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 6 } else { 3 };
    for _ in 0..count {
        let candidates: Vec<usize> = ctx
            .board
            .iter()
            .enumerate()
            .filter(|(_, u)| u.health > 0 && u.tribe.matches(Tribe::Dragon) && !u.divine_shield)
            .map(|(i, _)| i)
            .collect();
        if candidates.is_empty() {
            break;
        }
        let pick = if candidates.len() == 1 {
            candidates[0]
        } else {
            candidates[ctx.rng.below(candidates.len())]
        };
        ctx.board[pick].apply_keyword(Keyword::DivineShield, false);
    }
}
