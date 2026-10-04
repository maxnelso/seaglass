//! `Friendly Geist` (`BG32_880`) — Tier 4 Undead (`6/3`).
//!
//! Deathrattle: Your Tavern spells give an extra `+1` (`+2` if Golden) Attack this game.

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 421;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Friendly Geist", 6, 3, 4)
        .with_tribe(Tribe::Undead)
        .on_deathrattle(on_deathrattle)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let bonus = if dying.is_golden { 2 } else { 1 };
    ctx.auras.spell_bonus_atk += bonus;
}
