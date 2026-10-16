//! `Cursed Crystal` (`BG35_MagicItem_150`) — Lesser, 3 Gold, Demon.
//!
//! After the Tavern is Refreshed, give its minions +3/+3 this turn.
//!
//! `trinket_after_refresh`: the first Refresh of each turn (the turn start's too; `armed`, which
//! starts again each turn) gives the minions in the Tavern +3/+3 this turn, as `Ashen Corruptor`'s
//! (`auras.tavern_turn_atk` / `tavern_turn_hp`): so do the minions of the turn's later Refreshes.
//! A minion bought keeps it; one still in the Tavern loses it at the next turn start (confirmed).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131276;

/// The buff.
const BUFF: i32 = 3;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_after_refresh(after_refresh),
    )]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_turn(state, ID);
}

fn after_refresh(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let Some(t) = super::state_mut(&mut state.auras, ID).filter(|t| t.armed == 0) else {
        return;
    };
    t.armed = 1;
    state.auras.tavern_turn_atk += BUFF;
    state.auras.tavern_turn_hp += BUFF;
    for unit in state.shop.iter_mut().filter(|u| !u.is_spell) {
        unit.add_stats(BUFF, BUFF);
    }
}
