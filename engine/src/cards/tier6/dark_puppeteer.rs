//! `Dark Puppeteer` (`BG36_104`) — Tier 6 Aberration (`8/4`).
//!
//! Deathrattle: Your Tavern spells give an extra `+4` (`+8` if Golden) Health this game.

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 605;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Dark Puppeteer", 8, 4, 6)
        .with_tribe(Tribe::Aberration)
        .on_deathrattle(on_deathrattle)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let bonus = if dying.is_golden { 8 } else { 4 };
    ctx.auras.spell_bonus_hp += bonus;
}
