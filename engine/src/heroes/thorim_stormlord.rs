//! `Thorim, Stormlord` (`BG27_HERO_801`) — Armor 18 (Duos 15).
//!
//! Hero Power: `Choose Your Champion` (`BG27_HERO_801p2`), 0 Gold: Passive. At the start of the game, Discover a Tier 7 minion to get after you spend 60 Gold. (60 left!)
//!
//! Passive. At the start of the player's first turn (`hero_turn_start`, while nothing was offered:
//! `armed`; engine decision), a Discover among 3 different Tier 7 minions of the game's catalog
//! (not from the pool; one `rng.below(k)` each): the chosen one enters the hand locked
//! (`Unit::locked = true`, `not_from_pool = true`). `hero_gold_spent` counts the Gold spent
//! (`counter`); once it reaches 60, the locked champion in hand unlocks immediately (and runs its
//! normal Triple check).
//!
//! Master Nguyen never offers it for a single turn (`CardFlags::NOT_A_TURN_POWER`): its
//! start-of-game Discover would come back every turn.

use crate::cards::{self, CardFlags, CardHooks};
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
            .with_flags(CardFlags::NOT_A_TURN_POWER)
            .on_hero_turn_start(choose_champion)
            .on_hero_gold_spent(gold_spent),
    )]
}

/// The first turn: Discover a Tier 7 minion locked in your hand until 60 Gold is spent.
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
        option.locked = true;
        option.not_from_pool = true;
        options.push(option);
    }
    state.push_discover(options);
}

/// Gold spent: from 60 on, unlock the locked Tier 7 champion in hand.
fn gold_spent(state: &mut TavernState, amount: u32) {
    let reached = {
        let Some(power) = power_mut(&mut state.auras) else {
            return;
        };
        power.counter += amount as i32;
        power.counter >= GOLD
    };
    if !reached {
        return;
    }
    let mut unlocked = Vec::new();
    for card in &mut state.hand {
        if card.locked {
            card.locked = false;
            unlocked.push(card.card_id);
        }
    }
    for cid in unlocked {
        state.check_and_resolve_triple(cid);
    }
}
