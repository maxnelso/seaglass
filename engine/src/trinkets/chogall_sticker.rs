//! `Cho'gall Sticker` (`BGDUO_MagicItem_002`) — Greater, 5 Gold, Duos only.
//!
//! Whenever your team plays a Golden minion, both you and your teammate get a Triple Reward.
//!
//! `trinket_teammate`, after each of the team's actions: each Golden minion played from the hand
//! since, by the player or their teammate (each one's `auras.golden_minions_played`, the game's
//! count: Magnetized and intrinsic ones too **(engine decision)**), gives the player a `Triple
//! Reward` card of their Tier plus one, and the teammate one that locks in their own Tier as it
//! arrives (a `TeammateGift::Card`, as `Cho`'s Double Trouble). The counts are taken when it first
//! sees the teammate (`armed`), so earlier plays don't count.

use crate::cards::{tokens, CardHooks};
use crate::model::{CardId, TeammateGift};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};
use crate::trinkets::TeammateView;

pub const ID: CardId = 117933;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_teammate(teammate))]
}

fn teammate(state: &mut TavernState, mate: &TeammateView<'_>, _: &mut CardPool, _: &mut Rng) {
    let own = state.auras.golden_minions_played as i32;
    let theirs = mate.state.auras.golden_minions_played as i32;
    let Some(t) = super::state_mut(&mut state.auras, ID) else {
        return;
    };
    if t.armed == 0 {
        (t.armed, t.counter, t.stacks) = (1, own, theirs);
        return;
    }
    let played = (own - t.counter).max(0) + (theirs - t.stacks).max(0);
    (t.counter, t.stacks) = (own, theirs);
    for _ in 0..played {
        state.add_to_hand_or_wait(tokens::make_triple_reward(state.tavern_tier));
        state
            .auras
            .give_teammate(TeammateGift::Card(tokens::make_triple_reward(0)));
    }
}
