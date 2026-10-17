//! `Chromatic Tear (Greater)` (`BG35_MagicItem_840t`) — Greater, 1 Gold, Dragon.
//!
//! Get 2 random Chromadrakes. After you play 7 Battlecry minions, repeat this. (7 left!)
//!
//! Picked: 2 random `Chromadrake`s (as the Lesser one's). `trinket_after_play`: every 7th minion
//! with a Battlecry played from the hand (`counter`; Magnetized from it too) gives 2 more.

use crate::cards::{self, CardHooks, Played};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130977;

/// The Chromadrakes it gives each time.
const COUNT: usize = 2;
/// Battlecry minions to play per repeat.
const PLAYS: i32 = 7;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(give)
            .on_trinket_after_play(after_play),
    )]
}

fn give(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    super::chromatic_tear::give_chromadrakes(state, COUNT, rng);
}

fn after_play(state: &mut TavernState, played: &Played, pool: &mut CardPool, rng: &mut Rng) {
    if played.from_hand
        && cards::is_battlecry_minion(played.card_id)
        && super::count_to(&mut state.auras, ID, PLAYS)
    {
        give(state, pool, rng);
    }
}
