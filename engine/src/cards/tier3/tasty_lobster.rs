//! `Tasty Lobster` (`BG36_202`) — Tier 3 Beast (`2/1`).
//! **Deathrattle:** Give a random friendly Beast `+2/+1` (`+4/+2` if Golden). Improve your future Tasty Lobsters.

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 333;
pub const NAME: &str = "Tasty Lobster";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 1, 3)
        .with_tribe(Tribe::Beast)
        .on_deathrattle(on_deathrattle)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let mult = if dying.is_golden { 2 } else { 1 };
    let total_stacks = mult + ctx.auras.tasty_lobster_stacks;
    let d_atk = 2 * total_stacks;
    let d_hp = total_stacks;
    let beasts: Vec<usize> = ctx
        .board
        .iter()
        .enumerate()
        .filter(|(_, u)| u.tribe.matches(Tribe::Beast))
        .map(|(i, _)| i)
        .collect();
    if !beasts.is_empty() {
        let pick = if beasts.len() == 1 {
            beasts[0]
        } else {
            beasts[ctx.rng.below(beasts.len())]
        };
        ctx.buff_unit(pick, d_atk, d_hp, NAME);
    }
    ctx.auras.tasty_lobster_stacks += mult;
}
