//! `Jar o' Gems` (`BG30_MagicItem_546`) — Greater, 0 Gold, Quilboar.
//!
//! After 2 friendly minions attack each combat, this plays a Blood Gem on all your Quilboar. (2 left!)
//!
//! `trinket_friendly_attack`: the attacks of friendly minions in a combat count (`counter`, which
//! starts again each turn); once the 2nd has attacked, a Blood Gem is played (not cast:
//! `tavern.md` §7.4) on every friendly Quilboar, once per combat (**engine decision**: not every
//! 2 attacks). In the Tavern, an attack counts toward the coming combat's.

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, Tribe, UnitId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 112398;

/// Attacks before the Blood Gems.
const ATTACKS: i32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_friendly_attack(friendly_attack),
    )]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_combat(state, ID);
}

fn friendly_attack(ctx: &mut BoardCtx<'_>, _: UnitId) {
    let Some(trinket) = super::state_mut(ctx.auras, ID) else {
        return;
    };
    trinket.counter += 1;
    if trinket.counter != ATTACKS {
        return;
    }
    let auras = ctx.auras.clone();
    for unit in ctx.board.iter_mut() {
        if unit.health > 0 && unit.tribe.matches(Tribe::Quilboar) {
            unit.play_blood_gems(1, &auras);
        }
    }
}
