//! `The Jailer` (`TB_BaconShop_HERO_702`) — Armor 10 (Duos 8).
//!
//! Hero Power: `Rune of Damnation` (`TB_BaconShop_HP_702t`), 1 Gold: Destroy a friendly Undead to get a random Undead. (Unlocks at Tier 2.)
//!
//! Active, a friendly Undead, once per turn, from Tavern Tier 2 on: the Undead is destroyed
//! (`destroy_board_unit`: its Deathrattle resolves), then a random Undead of the player's Tier or
//! lower from the pool (engine decision) goes to the hand. It needs room in the hand.

use crate::cards::{ActivateTargetKind, CardHooks};
use crate::model::{CardId, HeroId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 97186;
pub const POWER: CardId = 122959;

/// The Tavern Tier it unlocks at.
const UNLOCK_TIER: u32 = 2;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(rune_of_damnation)
            .on_hero_power_usable(|state| {
                state.tavern_tier >= UNLOCK_TIER && super::hand_has_room(state)
            })
            .on_hero_power_target(|state, t| state.board[t].tribe.matches(Tribe::Undead))
            .with_activate_target(ActivateTargetKind::BoardAny),
    )]
}

/// Using it: destroy the Undead, get a random Undead.
fn rune_of_damnation(
    state: &mut TavernState,
    target: Option<usize>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let Some(t) = target.filter(|&t| t < state.board.len()) else {
        return;
    };
    state.destroy_board_unit(t, pool, rng);
    if let Some(undead) = pool.draw_by_tribe(Tribe::Undead, None, state.tavern_tier, rng) {
        state.add_pool_card_to_hand(undead, pool);
    }
}
