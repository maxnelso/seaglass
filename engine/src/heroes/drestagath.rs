//! `Drest'agath` (`BG36_HERO_000`) — Armor 12 (Duos 10).
//!
//! Hero Power: `Incubate` (`BG36_HERO_000p`), 1 Gold: Discard a card to get a random Aberration.
//!
//! Active, a card in the hand (not a locked one), once per turn: the card is discarded
//! (`discard_hand_card`: a minion's copies return to the pool, the discard observers see it), then
//! a random Aberration of the player's Tier or lower from the pool (engine decision) goes to the
//! hand.

use crate::cards::{ActivateTargetKind, CardHooks};
use crate::model::{CardId, HeroId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 134502;
pub const POWER: CardId = 134503;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(incubate)
            .with_activate_target(ActivateTargetKind::HandCard),
    )]
}

/// Using it: discard the card, get a random Aberration.
fn incubate(state: &mut TavernState, target: Option<usize>, pool: &mut CardPool, rng: &mut Rng) {
    let Some(t) = target else {
        return;
    };
    if state.discard_hand_card(t, pool, rng).is_none() {
        return;
    }
    if let Some(aberration) = pool.draw_by_tribe(Tribe::Aberration, None, state.tavern_tier, rng) {
        state.add_pool_card_to_hand(aberration, pool);
    }
}
