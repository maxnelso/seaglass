//! `Soul Battery` (`BG36_MagicItem_410`) — Lesser, 0 Gold, Undead.
//!
//! After you summon 25 minions, replace this with a random Greater Undead Trinket. (25 left!)
//!
//! Every friendly minion summoned counts (`counter`, across turns): in the Tavern (played, or
//! summoned by a card: `trinket_tavern_summon`) and in combat (`trinket_combat_summon`). Once 25
//! were summoned, it is replaced by a random Greater Undead trinket the player doesn't own
//! (`trinkets::replace_with_random`, `docs/trinkets.md` §3.5): after the Tavern action, or at
//! the next turn start for a count reached in combat **(engine decision)**.

use crate::cards::CardHooks;
use crate::model::{CardId, Tribe, TrinketState, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134438;

/// Summons until the replacement.
const SUMMONS: i32 = 25;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_tavern_summon(summon)
            .on_trinket_combat_summon(summon)
            .on_trinket_after_action(check)
            .on_trinket_turn_start(check),
    )]
}

fn summon(t: &mut TrinketState, _: &mut Unit) {
    t.counter = (t.counter + 1).min(SUMMONS);
}

fn check(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    super::replace_at_count(state, ID, SUMMONS, Tribe::Undead, pool, rng);
}
