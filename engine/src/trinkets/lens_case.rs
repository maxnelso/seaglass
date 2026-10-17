//! `Lens Case` (`BG35_MagicItem_817`) — Lesser, 3 Gold.
//!
//! Get a Duplicating Lens. At the start of every 2 turns, repeat this. (2 turns left!)
//!
//! Picked: `Duplicating Lens` (`BG35_MagicItem_817t`), a token spell, to the hand or waiting for
//! room; then every 2nd turn start (`counter`), another. Cast on a minion of Tier 3 or lower, a
//! friendly one or one in the Tavern: a plain copy of it (`cards::instantiate_plain_copy`) to the
//! hand.

use crate::cards::spells::targeted_minion;
use crate::cards::{self, CardFlags, CardHooks};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

pub const ID: CardId = 130835;
/// `Duplicating Lens`.
pub const SPELL: CardId = 130853;

/// Turns between Lenses.
const TURNS: i32 = 2;
/// The highest Tier it copies.
const MAX_TIER: u32 = 3;

/// The trinket's and its spell's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            ID,
            CardHooks::EMPTY
                .on_trinket_bought(give)
                .on_trinket_turn_start(turn_start),
        ),
        (
            SPELL,
            targeted_minion(cast)
                .on_spell_target_filter(|unit| unit.tavern_tier <= MAX_TIER)
                .with_flags(CardFlags::NOT_IN_POOL.union(CardFlags::NOT_TAVERN_SPELL)),
        ),
    ]
}

fn give(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.add_to_hand_or_wait(super::token_spell(SPELL, "Duplicating Lens"));
}

fn turn_start(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if super::count_to(&mut state.auras, ID, TURNS) {
        give(state, pool, rng);
    }
}

fn cast(state: &mut TavernState, _: &Unit, target: MinionSlot, _: &mut CardPool, _: &mut Rng) {
    let copy = state
        .minion_mut(target)
        .map(|u| cards::instantiate_plain_copy(u));
    if let Some(copy) = copy {
        state.add_to_hand(copy);
    }
}
