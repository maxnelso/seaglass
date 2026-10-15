//! `Sylvanas Windrunner` (`BG23_HERO_306`) — Armor 10 (Duos 8).
//!
//! Hero Power: `Reclaimed Souls` (`BG23_HERO_306p`), 2 Gold: Discover a plain copy of a minion that died last combat. (Unlocks on Turn 3.)
//!
//! Active, untargeted, once per turn, from the 3rd turn on: `hero_after_combat` keeps plain copies
//! of the player's minions that died in their last combat (`stored`, replaced each combat); the
//! power Discovers among up to 3 of them with different cards, at random (not from the pool). It
//! needs one and room in the hand.

use crate::cards::{self, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 89293;
pub const POWER: CardId = 89294;

/// The turn it unlocks on.
const UNLOCK_TURN: u32 = 3;

/// Discover options.
const OPTIONS: usize = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(reclaimed_souls)
            .on_hero_power_usable(|state| {
                state.turn >= UNLOCK_TURN
                    && super::hand_has_room(state)
                    && state
                        .auras
                        .hero
                        .as_ref()
                        .is_some_and(|h| !h.power.stored.is_empty())
            })
            .on_hero_after_combat(after_combat),
    )]
}

/// After a combat: plain copies of the minions that died in it.
fn after_combat(state: &mut TavernState, dead: &[Unit]) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.stored = dead.iter().filter_map(cards::plain_instance).collect();
    }
}

/// Using it: Discover among them, different cards.
fn reclaimed_souls(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, rng: &mut Rng) {
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    let mut candidates: Vec<Unit> = Vec::new();
    for unit in &power.stored {
        if !candidates.iter().any(|c| c.card_id == unit.card_id) {
            candidates.push(unit.clone());
        }
    }
    let mut options = Vec::new();
    while options.len() < OPTIONS && !candidates.is_empty() {
        let pick = match candidates.len() {
            1 => 0,
            k => rng.below(k),
        };
        options.push(candidates.remove(pick));
    }
    for option in &mut options {
        state.apply_global_unit_auras(option);
    }
    state.push_discover(options);
}
