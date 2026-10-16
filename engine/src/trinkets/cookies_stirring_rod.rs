//! `Cookie's Stirring Rod` (`BG36_MagicItem_850`) — Greater, 0 Gold, Murloc.
//!
//! After you play 5 Murlocs, get two random Tavern spells. (5 left!)
//!
//! `trinket_after_play`: every 5th Murloc played from the hand (`counter`, across turns) gives 2
//! random Tavern spells, drawn as the shop draws one, which wait for room in a full hand.

use crate::cards::{spells, CardHooks, Played};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133070;

/// Murlocs to play per reward.
const MURLOCS: i32 = 5;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_play(after_play))]
}

fn after_play(state: &mut TavernState, played: &Played, _: &mut CardPool, rng: &mut Rng) {
    if !played.from_hand || played.magnetized || !played.tribe.matches(Tribe::Murloc) {
        return;
    }
    if super::count_to(&mut state.auras, ID, MURLOCS) {
        for _ in 0..2 {
            let spell = spells::draw_shop_spell(state.tavern_tier, state.auras.duos_game, rng);
            state.add_to_hand_or_wait(spell);
        }
    }
}
