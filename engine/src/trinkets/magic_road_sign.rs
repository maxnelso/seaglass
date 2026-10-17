//! `Magic Road Sign` (`BGDUO_MagicItem_006`) — Greater, 3 Gold, Duos only.
//!
//! After you sell your first minion each turn, Pass it.
//!
//! `trinket_after_sell`: the first minion sold each turn (`armed`), once its sell triggers
//! resolved, is Passed to the teammate as it was sold, rather than going back to the pool (its
//! copies leave the pool again): a Pass by a card effect (`TeammateGift::Pass`: free, counted as
//! the team's; `docs/duos_tavern.md` §7.2). The sale's Gold stays.

use crate::cards::CardHooks;
use crate::model::{CardId, TeammateGift, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 117937;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_after_sell(after_sell),
    )]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_turn(state, ID);
}

fn after_sell(state: &mut TavernState, sold: &Unit, pool: &mut CardPool, _: &mut Rng) {
    let Some(t) = super::state_mut(&mut state.auras, ID).filter(|t| t.armed == 0) else {
        return;
    };
    t.armed = 1;
    pool.take_unit(sold);
    state.auras.give_teammate(TeammateGift::Pass(sold.clone()));
}
