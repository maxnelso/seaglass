//! `Deathwing` (`TB_BaconShop_HERO_52`) — Armor 18 (Duos 16).
//!
//! Hero Power: `ALL Will Burn!` (`TB_BaconShop_HP_061`), 0 Gold: Start of Combat: Give ALL minions +2 Attack permanently.
//!
//! Passive. At Start of Combat every minion on both boards gains +2 Attack, the player's left to
//! right, then the enemy's (engine decision); it is permanent on both sides (confirmed): the
//! minions' Tavern copies keep it (`perm_atk_gained`, `docs/heroes.md` §7.4). Minions summoned
//! later don't get it.

use crate::cards::{CardHooks, HeroCombatCtx};
use crate::model::{CardId, HeroId};

pub const ID: HeroId = 60369;
pub const POWER: CardId = 61406;

const NAME: &str = "ALL Will Burn!";

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY.on_hero_start_of_combat(start_of_combat),
    )]
}

/// Start of Combat: ALL minions gain +2 Attack, permanently.
fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    for i in 0..ctx.own.board.len() {
        ctx.own.buff_unit(i, 2, 0, NAME);
        ctx.own.board[i].perm_atk_gained += 2;
    }
    for i in 0..ctx.enemy_board.len() {
        ctx.buff_enemy(i, 2, 0, NAME);
        ctx.enemy_board[i].perm_atk_gained += 2;
    }
}
