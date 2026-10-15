//! `A. F. Kay` (`TB_BaconShop_HERO_16`) — Armor 15 (Duos 13).
//!
//! Hero Power: `Procrastinate` (`TB_BaconShop_HP_044`), 0 Gold: Skip your first two turns, then Discover a minion from Tier 3 and Tier 4.
//!
//! Passive. `hero_skips_turn` on turns 1 and 2 (only `EndTurn` is legal). At the start of turn 3
//! (`hero_turn_start`): two `Triple Reward` cards, locked in at Tier 3 and at Tier 4 (each lost
//! with a full hand, `docs/tavern.md` §5.2; confirmed).

use crate::cards::{tokens, CardHooks};
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 57944;
pub const POWER: CardId = 59891;

/// The turns skipped.
const SKIPPED: u32 = 2;

/// The `Triple Reward` cards' Tiers, in order.
const TIERS: [u32; 2] = [3, 4];

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_skips_turn(|state| (1..=SKIPPED).contains(&state.turn))
            .on_hero_turn_start(turn_start),
    )]
}

/// The first played turn: a Tier 3 and a Tier 4 `Triple Reward` card.
fn turn_start(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    if state.turn != SKIPPED + 1 {
        return;
    }
    for tier in TIERS {
        // (`make_triple_reward` locks in the next Tier of the one it is given.)
        state.add_to_hand(tokens::make_triple_reward(tier - 1));
    }
}
