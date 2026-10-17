//! `Tras'tath, Soul Parasite` (`BG36_HERO_101`) — Armor 15 (Duos 13).
//!
//! Hero Power: `Void Power` (`BG36_HERO_101p`), 0 Gold: At the start of the game, Discover a Tier 5 minion with a Dark Gift. It unlocks on Turn 7. (6 turns left!)
//!
//! Passive. On the first turn (`hero_turn_start`), an offer of 3 Tier 5 minions from the pool,
//! each with a different gift of Tras'tath's pool (the gifts flagged `trastath`), whatever their
//! turn windows, by turn 7's other rules (engine decision). The options are locked
//! (`Unit::locked_turns`) until turn 7: the chosen one can't be played, sold from the hand or
//! Passed before (engine decision; `docs/dark_gifts.md` §12.2). Offered only in games with Dark
//! Gifts (`CardFlags::DARK_GIFTS`); never by a Discover of Hero Powers
//! (`CardFlags::NOT_DISCOVERABLE_POWER`): its start of game would be past.

use crate::cards::{CardFlags, CardHooks};
use crate::dark_gifts::{self, GiftPool, OfferRules, TypeRule};
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 132578;
pub const POWER: CardId = 132581;

/// The turn the minion unlocks on.
const UNLOCK_TURN: u32 = 7;

/// The Tier of the minions offered.
const TIER: u32 = 5;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .with_flags(CardFlags::DARK_GIFTS | CardFlags::NOT_DISCOVERABLE_POWER)
            .on_hero_turn_start(turn_start),
    )]
}

/// The first turn: the locked Tier 5 offer.
fn turn_start(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if state.turn != 1 {
        return;
    }
    let rules = OfferRules {
        turn: UNLOCK_TURN,
        tiers: vec![TIER],
        most_common: TypeRule::None,
        gifts: GiftPool::Trastath,
        golden: false,
    };
    let mut options = dark_gifts::draw_offer(state, pool, rng, &rules);
    for option in &mut options {
        option.locked_turns = UNLOCK_TURN - state.turn;
    }
    state.push_discover(options);
}
