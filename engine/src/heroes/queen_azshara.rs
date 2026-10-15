//! `Queen Azshara` (`BG22_HERO_007`) — Armor 12 (Duos 10).
//!
//! Hero Power: `Azshara's Ambition` (`BG22_HERO_007p`), 0 Gold: When your warband reaches 30 total Attack, begin your Naga Conquest.
//!
//! Passive (`docs/heroes.md` §11.2): whenever the friendly board's total Attack is 30 or more
//! (`hero_board_changed`), the Hero Power transforms into `Naga Conquest` (`BG22_HERO_007p2`,
//! 1 Gold active, immediately usable that turn: Discover a Naga of the player's Tier or lower from
//! the pool).

use crate::cards::CardHooks;
use crate::heroes::{hand_has_room, power_mut};
use crate::model::{CardId, HeroId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 79618;
pub const POWER: CardId = 79619;

/// `Naga Conquest` (`BG22_HERO_007p2`).
pub const NAGA_CONQUEST: CardId = 80007;

const AMBITION_ATTACK: i32 = 30;
const OPTIONS: usize = 3;

pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            POWER,
            CardHooks::EMPTY
                .on_hero_chosen(check_ambition)
                .on_hero_board_changed(check_ambition),
        ),
        (
            NAGA_CONQUEST,
            CardHooks::EMPTY
                .on_hero_power(naga_conquest)
                .on_hero_power_usable(hand_has_room),
        ),
    ]
}

fn check_ambition(state: &mut TavernState) {
    let total_attack: i32 = state.board.iter().map(|u| u.attack.max(0)).sum();
    if total_attack < AMBITION_ATTACK {
        return;
    }
    if let Some(power) = power_mut(&mut state.auras) {
        power.card_id = NAGA_CONQUEST;
        power.cost = 1;
        power.uses_left = 1;
        power.counter = 0;
    }
}

fn naga_conquest(state: &mut TavernState, _: Option<usize>, pool: &mut CardPool, rng: &mut Rng) {
    let options = pool.draw_discover_by_tribe(Tribe::Naga, state.tavern_tier, OPTIONS, rng);
    state.push_discover(options);
}
