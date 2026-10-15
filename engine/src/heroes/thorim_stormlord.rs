//! `Thorim, Stormlord` (`BG27_HERO_801`) — Armor 18 (Duos 15).
//!
//! Hero Power: `Choose Your Champion` (`BG27_HERO_801p2`), 0 Gold: Passive. At the start of the game, Discover a Tier 7 minion to get after you spend 60 Gold. (60 left!)
//!
//! Passive. At the start of the player's first turn (`hero_turn_start`, while nothing was offered:
//! `armed`; engine decision), a Discover among 3 different Tier 7 minions of the game's catalog
//! (not the pool; one `rng.below(k)` each) whose options go to the Hero Power
//! (`Unit::to_hero_power`, `docs/tavern.md` §5.2): the chosen one is kept (`stored`).
//! `hero_gold_spent` counts the Gold spent (`counter`); from 60 on, the kept minion goes to the hand
//! as soon as it has room (engine decision).

use crate::cards::{self, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 103477;
pub const POWER: CardId = 104628;

/// The Gold to spend.
const GOLD: i32 = 60;

/// The champions' Tier.
const TIER: u32 = 7;

/// Discover options.
const OPTIONS: usize = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_turn_start(choose_champion)
            .on_hero_gold_spent(gold_spent),
    )]
}

/// The first turn: Discover a Tier 7 minion for the Hero Power to keep.
fn choose_champion(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    let Some(power) = power_mut(&mut state.auras).filter(|p| p.armed == 0) else {
        return;
    };
    power.armed = 1;
    let mut champions: Vec<_> = cards::game_templates(&state.auras)
        .iter()
        .filter(|t| t.tavern_tier == TIER)
        .collect();
    let mut options = Vec::new();
    while options.len() < OPTIONS && !champions.is_empty() {
        let pick = match champions.len() {
            1 => 0,
            k => rng.below(k),
        };
        let mut option = champions.remove(pick).instantiate();
        option.to_hero_power = true;
        options.push(option);
    }
    state.push_discover(options);
}

/// Gold spent: from 60 on, the kept champion arrives once the hand has room.
fn gold_spent(state: &mut TavernState, amount: u32) {
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    power.counter += amount as i32;
    if power.counter < GOLD || power.stored.is_empty() || !super::hand_has_room(state) {
        return;
    }
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    let champion = power.stored.remove(0);
    state.add_to_hand(champion);
}
