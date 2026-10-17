//! `Quartermaster's Hook` (`BG36_MagicItem_408`) — Lesser, 0 Gold, Pirate.
//!
//! After you spend 50 Gold, replace this with a random Greater Pirate Trinket. (50 Gold left!)
//!
//! `trinket_gold_spent`: every Gold spent counts (`counter`, across turns); at 50 it is replaced
//! by a random Greater Pirate trinket the player doesn't own (`trinkets::replace_with_random`,
//! `docs/trinkets.md` §3.5), at once.

use crate::cards::CardHooks;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134436;

/// Gold spent until the replacement.
const GOLD: i32 = 50;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_gold_spent(gold_spent))]
}

fn gold_spent(state: &mut TavernState, amount: u32, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        t.counter = t
            .counter
            .saturating_add(amount.min(GOLD as u32) as i32)
            .min(GOLD);
    }
    super::replace_at_count(state, ID, GOLD, Tribe::Pirate, pool, rng);
}
