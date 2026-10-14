//! `Rokara` (`BG20_HERO_100`) — Armor 18 (Duos 16).
//!
//! Hero Power: `Glory of Combat` (`BG20_HERO_100p`), 0 Gold: After a friendly minion kills an enemy, give it +1 Attack permanently.
//!
//! Passive. When a friendly minion's damage kills an enemy minion in combat (`hero_enemy_killed`,
//! `docs/heroes.md` §7.3), the killer gains +1 Attack, permanently (its Tavern copy keeps it), if
//! it is still alive: a killer that died in the same exchange gains nothing (engine decision).

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, HeroId, Unit, UnitId};

pub const ID: HeroId = 70838;
pub const POWER: CardId = 80229;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_enemy_killed(enemy_killed))]
}

/// A friendly minion killed an enemy: it gains +1 Attack permanently.
fn enemy_killed(ctx: &mut BoardCtx<'_>, killer: UnitId, _: &Unit) {
    if let Some(i) = ctx
        .board
        .iter()
        .position(|u| u.id == killer && u.health > 0)
    {
        ctx.buff_unit(i, 1, 0, "Glory of Combat");
        ctx.board[i].perm_atk_gained += 1;
    }
}
