//! `Xyrella` (`BG20_HERO_101`) — Armor 14 (Duos 12).
//!
//! Hero Power: `See the Light` (`BG20_HERO_101p`), 2 Gold: Choose a minion in the Tavern. Set its stats to 2 and add it to your hand.
//!
//! Active, a minion in the Tavern, once per turn: its Attack and Health (and their maximums)
//! become 2, net of the auras on it (`aura_applied` clears, as for `Robust Evolution`), and it goes
//! to the hand (not a buy), where the auras it has apply again (a +0/+3 Volumizer bonus makes it
//! 2/5; engine decision). It needs room in the hand; a minion the hand has
//! no room for by then (a Gold-spent effect filled it) stays in the Tavern as it is (engine
//! decision).

use crate::cards::{ActivateTargetKind, CardHooks};
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 70956;
pub const POWER: CardId = 70957;

/// The stats it sets.
const STATS: i32 = 2;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(see_the_light)
            .on_hero_power_usable(super::hand_has_room)
            .on_hero_power_target(|state, t| !state.shop[t].is_spell)
            .with_activate_target(ActivateTargetKind::ShopCard),
    )]
}

/// Using it: the minion, set to 2/2, to the hand.
fn see_the_light(state: &mut TavernState, target: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    let Some(t) = target.filter(|&t| t < state.shop.len() && super::hand_has_room(state)) else {
        return;
    };
    let mut minion = state.shop.remove(t);
    minion.attack = STATS;
    minion.health = STATS;
    minion.max_attack = STATS;
    minion.max_health = STATS;
    // Set net of its auras: they apply again as it reaches the hand, where it has them.
    minion.aura_applied = (0, 0);
    state.add_to_hand(minion);
}
