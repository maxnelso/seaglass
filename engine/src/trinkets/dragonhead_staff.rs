//! `Dragonhead Staff` (`BG36_MagicItem_407`) — Lesser, 0 Gold, Dragon.
//!
//! After 13 friendly minions attack, replace this with a random Greater Dragon Trinket. (13 left!)
//!
//! `trinket_friendly_attack`: each friendly attack, in combat or in the Tavern (a Fishbait),
//! counts (`counter`, across combats). Once 13 have attacked, it is replaced by a random Greater
//! Dragon trinket the player doesn't own (`trinkets::replace_with_random`, `docs/trinkets.md`
//! §3.5): after the Tavern action, or at the next turn start for a count reached in combat
//! **(engine decision)**.

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, Tribe, UnitId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134435;

/// Friendly attacks until the replacement.
const ATTACKS: i32 = 13;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_friendly_attack(friendly_attack)
            .on_trinket_after_action(check)
            .on_trinket_turn_start(check),
    )]
}

fn friendly_attack(ctx: &mut BoardCtx<'_>, _: UnitId) {
    if let Some(t) = super::state_mut(ctx.auras, ID) {
        t.counter = (t.counter + 1).min(ATTACKS);
    }
}

fn check(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    super::replace_at_count(state, ID, ATTACKS, Tribe::Dragon, pool, rng);
}
