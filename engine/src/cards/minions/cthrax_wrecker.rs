//! `C'Thrax Wrecker` (`BGDUO_701`) — Tier 5 Aberration (`8/8`), Duos only.
//! **Battlecry, Deathrattle, and Rally:** Give your team's **Deities** `+4/+4` (Golden
//! `BGDUO_701_G`, `16/16`: `+8/+8`).
//!
//! The player's Deity gains it at once (in combat, an awakened Deity on the board too, as
//! `Drifting Sacrifice`); the teammate's through a gift (`docs/duos_tavern.md` §7.2), in combat
//! after the combat.

use crate::cards::{CardTemplate, DeathrattleContext, RallyCtx};
use crate::model::{CardId, PlayerAuras, TeammateGift, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10501;
pub const NAME: &str = "C'Thrax Wrecker";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 8, 8, 5)
        .with_tribe(Tribe::Aberration)
        .on_battlecry(on_battlecry)
        .on_deathrattle(on_deathrattle)
        .on_rally(on_rally)
        .on_teammate_gift(teammate_gift)
}

/// Give the player's Deity `+buff/+buff`, and the teammate's through a gift.
fn buff_team_deities(auras: &mut PlayerAuras, buff: i32) {
    auras.deity.attack += buff;
    auras.deity.health += buff;
    auras.give_teammate(TeammateGift::Effect {
        card_id: ID,
        amount: buff,
        team: false,
    });
}

pub fn on_battlecry(
    state: &mut TavernState,
    unit: &mut Unit,
    _: usize,
    _: &mut CardPool,
    _: &mut Rng,
) {
    buff_team_deities(&mut state.auras, 4 * unit.golden_mult());
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let buff = 4 * dying.golden_mult();
    buff_team_deities(ctx.auras, buff);
    for idx in 0..ctx.board.len() {
        if ctx.board[idx].is_deity {
            ctx.buff_unit(idx, buff, buff, NAME);
        }
    }
}

pub fn on_rally(ctx: &mut RallyCtx<'_>) -> Vec<Unit> {
    let buff = if ctx.is_golden { 8 } else { 4 };
    buff_team_deities(ctx.auras, buff);
    let mut cast = ctx.cast_ctx();
    for idx in 0..cast.board.len() {
        if cast.board[idx].is_deity && cast.board[idx].health > 0 {
            cast.buff_unit(idx, buff, buff, NAME);
        }
    }
    Vec::new()
}

/// The teammate's Tavern: their Deity gains `+amount/+amount`.
pub fn teammate_gift(state: &mut TavernState, amount: i32, _: &mut Rng) -> Vec<Unit> {
    state.auras.deity.attack += amount;
    state.auras.deity.health += amount;
    Vec::new()
}
