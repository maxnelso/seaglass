//! `Common Thread` (`BGDUO_MagicItem_001`) — Lesser, 4 Gold, Duos only.
//!
//! Pass a copy of the first card you buy each turn.
//!
//! `trinket_after_buy`: the first card bought each turn (minion or spell; `armed`) is Passed, as a
//! copy of it as bought, to the teammate: a Pass by a card effect (`TeammateGift::Pass`: free,
//! counted as the team's; `docs/duos_tavern.md` §7.2). The bought card stays in the hand.

use crate::cards::CardHooks;
use crate::model::{CardId, TeammateGift, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 117932;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_after_buy(after_buy),
    )]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_turn(state, ID);
}

fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, _: &mut Rng) {
    let Some(t) = super::state_mut(&mut state.auras, ID).filter(|t| t.armed == 0) else {
        return;
    };
    t.armed = 1;
    state
        .auras
        .give_teammate(TeammateGift::Pass(bought.clone()));
}
