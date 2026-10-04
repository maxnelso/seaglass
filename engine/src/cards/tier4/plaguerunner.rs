//! `Plaguerunner` (`BG34_690`) — Tier 4 Undead (`4/2`).
//!
//! Deathrattle: Your Undead have `+2` (`+4` if Golden) Attack this game, wherever they are.
//! (`+4` / `+8` if triggered outside combat!)

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 446;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Plaguerunner", 4, 2, 4)
        .with_tribe(Tribe::Undead)
        .on_deathrattle(on_deathrattle)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let base = if ctx.in_combat { 2 } else { 4 };
    let bonus = if dying.is_golden { base * 2 } else { base };
    ctx.auras.undead_bonus_attack += bonus;
}
