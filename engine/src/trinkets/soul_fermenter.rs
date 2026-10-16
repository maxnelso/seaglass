//! `Soul Fermenter` (`BG35_MagicItem_732`) — Greater, 6 Gold, Undead.
//!
//! Start of Combat: Destroy your 3 left-most minions. After your last minion dies, resummon them.
//!
//! `trinket_start_of_combat`: the 3 left-most friendly minions (fewer if there are fewer) are
//! destroyed (their Deathrattles trigger), and the trinket keeps them as they were (`stored`).
//! `trinket_friendly_death`: once no friendly minion is left alive after a round of deaths (the
//! Start of Combat's own, if it destroyed them all), they are summoned again, as kept, at the right
//! end, once per combat **(engine decision)**. What is kept is cleared every combat and turn.

use crate::cards::{BoardCtx, CardHooks, HeroCombatCtx};
use crate::model::{CardId, TrinketState, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131132;

/// Minions destroyed.
const COUNT: usize = 3;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_combat_start(combat_start)
            .on_trinket_turn_start(new_turn)
            .on_trinket_start_of_combat(start_of_combat)
            .on_trinket_friendly_death(friendly_death),
    )]
}

fn combat_start(trinket: &mut TrinketState) {
    trinket.stored.clear();
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        t.stored.clear();
    }
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let doomed: Vec<Unit> = ctx
        .own
        .board
        .iter()
        .filter(|u| u.health > 0)
        .take(COUNT)
        .cloned()
        .collect();
    ctx.own.destroys.extend(doomed.iter().map(|u| u.id));
    if let Some(t) = super::state_mut(ctx.own.auras, ID) {
        t.stored = doomed;
    }
}

fn friendly_death(ctx: &mut BoardCtx<'_>, _: &Unit) {
    if ctx.board.iter().any(|u| u.health > 0) {
        return;
    }
    let Some(t) = super::state_mut(ctx.auras, ID) else {
        return;
    };
    let kept = std::mem::take(&mut t.stored);
    for unit in kept {
        ctx.cursor = ctx.board.len();
        ctx.summon_as(unit.id, unit, "Soul Fermenter");
    }
}
