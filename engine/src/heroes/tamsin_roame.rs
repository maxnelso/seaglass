//! `Tamsin Roame` (`BG20_HERO_282`) — Armor 10 (Duos 8).
//!
//! Hero Power: `Fragrant Phylactery` (`BG20_HERO_282p`), 0 Gold: Start of Combat: Give your lowest-Attack minion "Deathrattle: Give your other minions this minion's stats."
//!
//! Passive. At Start of Combat the left-most of the minions with the lowest Attack (engine
//! decision) gains the Deathrattle (an extra Deathrattle, `docs/heroes.md` §4). When it dies, the
//! player's other minions on the board gain its Attack and its Health (`max_health`: the Health it
//! had before damage; engine decision).

use crate::cards::{BoardCtx, CardHooks, HeroCombatCtx};
use crate::model::{CardId, HeroId, Unit};

pub const ID: HeroId = 74646;
pub const POWER: CardId = 77911;

/// The Deathrattle it gives (`BG20_HERO_282pe`, "Fragrant").
pub const PHYLACTERY: CardId = 77912;

/// The Hero Power's hooks and its Deathrattle's, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            POWER,
            CardHooks::EMPTY.on_hero_start_of_combat(start_of_combat),
        ),
        (PHYLACTERY, CardHooks::EMPTY.on_deathrattle(phylactery)),
    ]
}

/// Start of Combat: the lowest-Attack minion gains the Deathrattle.
fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let Some(lowest) = ctx.own.board.iter().map(|u| u.attack).min() else {
        return;
    };
    if let Some(unit) = ctx.own.board.iter_mut().find(|u| u.attack == lowest) {
        unit.extra_deathrattles.push(PHYLACTERY);
    }
}

/// Deathrattle: the player's other minions gain this minion's stats.
fn phylactery(dying: &Unit, ctx: &mut BoardCtx<'_>) {
    let (attack, health) = (dying.attack.max(0), dying.max_health.max(0));
    for i in 0..ctx.board.len() {
        if ctx.board[i].id != dying.id {
            ctx.buff_unit(i, attack, health, "Fragrant Phylactery");
        }
    }
}
