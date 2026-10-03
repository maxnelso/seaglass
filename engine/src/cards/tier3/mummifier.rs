//! `Mummifier` (`BG28_309`) — Tier 3 Undead (`5/2`).
//! **Deathrattle:** Give a (`2` if Golden) different friendly Undead **Reborn**.

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, Keyword, Tribe, Unit};

pub const ID: CardId = 325;
pub const NAME: &str = "Mummifier";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 5, 2, 3).with_tribe(Tribe::Undead)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let mut candidates: Vec<usize> = ctx
        .board
        .iter()
        .enumerate()
        .filter(|(_, u)| u.id != dying.id && u.tribe.matches(Tribe::Undead) && !u.reborn)
        .map(|(i, _)| i)
        .collect();
    let count = if dying.is_golden { 2 } else { 1 };
    let n = candidates.len().min(count);
    for _ in 0..n {
        let pick = if candidates.len() == 1 {
            0
        } else {
            ctx.rng.below(candidates.len())
        };
        let b_idx = candidates.remove(pick);
        ctx.board[b_idx].apply_keyword(Keyword::Reborn, false);
    }
}
