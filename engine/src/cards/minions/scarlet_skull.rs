//! `Scarlet Skull` (`BG25_022`) — Tier 2 Undead (`2/1`).
//! **Reborn**. **Deathrattle:** Give a friendly Undead `+1/+2` (`+2/+4` if Golden).

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, Keyword, Tribe, Unit};

pub const ID: CardId = 225;
pub const NAME: &str = "Scarlet Skull";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 1, 2)
        .with_tribe(Tribe::Undead)
        .with_keyword(Keyword::Reborn)
        .on_deathrattle(on_deathrattle)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let candidates: Vec<usize> = ctx
        .board
        .iter()
        .enumerate()
        .filter(|(_, u)| u.tribe.matches(Tribe::Undead))
        .map(|(i, _)| i)
        .collect();
    if candidates.is_empty() {
        return;
    }
    let idx = if candidates.len() == 1 {
        candidates[0]
    } else {
        candidates[ctx.rng.below(candidates.len())]
    };
    let mult = if dying.is_golden { 2 } else { 1 };
    ctx.buff_unit(idx, mult, 2 * mult, "ScarletSkull");
}
