//! `Jewelry Box` (`BG35_MagicItem_434`) — Lesser, 0 Gold, Quilboar.
//!
//! Get a Blood Gem that gives a Quilboar Taunt, Divine Shield, or Reborn. At the start of each turn, repeat this.
//!
//! Picked, and at each turn start: a `Blood Gem` whose keyword is drawn now (Taunt, Divine Shield
//! or Reborn, one `rng.below(3)`: **engine decision**) goes to the hand, or waits for room in a
//! full hand. It is a Blood Gem in every way, its `counter` naming the keyword (1 Taunt, 2 Divine
//! Shield, 3 Reborn: `blood_gem::gem_keyword`); played on a Quilboar, each cast also gives it that
//! keyword.

use crate::cards::{tokens, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130904;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(give)
            .on_trinket_turn_start(give),
    )]
}

fn give(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    let mut gem = tokens::make_blood_gem();
    gem.counter = 1 + rng.below(3) as i32;
    state.add_to_hand_or_wait(gem);
}
