//! `Sky Golem Portrait` (`BG35_MagicItem_740`) — Greater, 3 Gold, Mech.
//!
//! Get a Falling Sky Golem. Start of Combat: Give your minions "Deathrattle: Give your minions +2/+2 permanently."
//!
//! Picked: a plain `Falling Sky Golem`. Step 1c: every friendly minion gains the extra Deathrattle
//! *Skyward* (`BG35_MagicItem_740e`, `Unit::extra_deathrattles`) for the combat: the friendly
//! minions alive then gain +2/+2, kept after the combat (`perm_atk_gained`, as `Devout
//! Hellcaller`'s).

use crate::cards::minions::falling_sky_golem;
use crate::cards::{BoardCtx, CardHooks, HeroCombatCtx};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131138;
/// *Skyward*: the Deathrattle the trinket gives.
pub const SKYWARD: CardId = 131142;

/// The stats the Deathrattle gives.
const STATS: i32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            ID,
            CardHooks::EMPTY
                .on_trinket_bought(bought)
                .on_trinket_start_of_combat(start_of_combat),
        ),
        (SKYWARD, CardHooks::EMPTY.on_deathrattle(skyward)),
    ]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, falling_sky_golem::ID);
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    for unit in ctx.own.board.iter_mut() {
        unit.extra_deathrattles.push(SKYWARD);
    }
}

fn skyward(_: &Unit, ctx: &mut BoardCtx<'_>) {
    for idx in 0..ctx.board.len() {
        if ctx.board[idx].health <= 0 {
            continue;
        }
        ctx.board[idx].perm_atk_gained += STATS;
        ctx.board[idx].perm_hp_gained += STATS;
        ctx.buff_unit(idx, STATS, STATS, "Skyward");
    }
}
