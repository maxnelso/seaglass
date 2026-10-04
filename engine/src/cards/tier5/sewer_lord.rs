//! `Sewer Lord` (`BG35_604`) — Tier 5 Beast (`4/6`).
//!
//! Deathrattle: Summon two `Sewer Rat`s (or two Golden `Sewer Rat`s if Golden) that summon `2/3` (`4/6`) Turtles with Taunt.

use crate::cards::{tokens, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 545;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Sewer Lord", 4, 6, 5).with_tribe(Tribe::Beast)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    for _ in 0..2 {
        ctx.summon(dying.id, tokens::make_sewer_rat(dying.is_golden));
    }
}

pub fn on_sewer_rat_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    ctx.summon(dying.id, tokens::make_half_shell(dying.is_golden));
}
