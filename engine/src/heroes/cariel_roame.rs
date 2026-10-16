//! `Cariel Roame` (`BG21_HERO_000`) — Armor 10 (Duos 8).
//!
//! Hero Power: `Conviction` (`BG21_HERO_000p`), 0 Gold: Give 1 random friendly minion +1/+1. Passive: After each combat, choose an improvement.
//!
//! Active, untargeted, 0 Gold (`docs/heroes.md` §11.2): gives `1 + armed` distinct random friendly
//! minions `+(1 + counter)/+(1 + stacks)`. After each combat (`hero_after_combat`), offers a Choose
//! One of `Offensive Stance` (+1 Attack: `counter += 1`), `Defensive Stance` (+1 Health:
//! `stacks += 1`), and `United Stance` (+1 target: `armed += 1`).
//!
//! Master Nguyen never offers it for a single turn (`CardFlags::NOT_A_TURN_POWER`, confirmed).

use crate::cards::{tokens, CardFlags, CardHooks};
use crate::heroes::{power_mut, power_mut_for};
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 73940;
pub const POWER: CardId = 73941;

pub const OFFENSIVE_STANCE: CardId = 101567;
pub const DEFENSIVE_STANCE: CardId = 101575;
pub const UNITED_STANCE: CardId = 104923;

const STANCES: [(CardId, &str); 3] = [
    (OFFENSIVE_STANCE, "Offensive Stance"),
    (DEFENSIVE_STANCE, "Defensive Stance"),
    (UNITED_STANCE, "United Stance"),
];

pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            POWER,
            CardHooks::EMPTY
                .with_flags(CardFlags::NOT_A_TURN_POWER)
                .on_hero_power(conviction)
                .on_hero_power_usable(usable)
                .on_hero_after_combat(after_combat),
        ),
        (
            OFFENSIVE_STANCE,
            CardHooks::EMPTY.on_chosen(choose_offensive),
        ),
        (
            DEFENSIVE_STANCE,
            CardHooks::EMPTY.on_chosen(choose_defensive),
        ),
        (UNITED_STANCE, CardHooks::EMPTY.on_chosen(choose_united)),
    ]
}

fn usable(state: &TavernState) -> bool {
    !state.board.is_empty()
}

fn conviction(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, rng: &mut Rng) {
    let Some((targets, atk, hp)) = power_mut(&mut state.auras).map(|p| {
        (
            1 + p.armed as usize,
            1 + p.counter.max(0),
            1 + p.stacks.max(0),
        )
    }) else {
        return;
    };
    let mut candidates: Vec<usize> = (0..state.board.len()).collect();
    let count = targets.min(candidates.len());
    let picked: Vec<usize> = if candidates.len() <= count {
        candidates
    } else {
        (0..count)
            .map(|_| candidates.remove(rng.below(candidates.len())))
            .collect()
    };
    for idx in picked {
        state.board[idx].add_stats(atk, hp);
    }
}

fn after_combat(state: &mut TavernState, _: &[Unit]) {
    let options = STANCES
        .iter()
        .map(|&(id, name)| tokens::make_choice_option(id, name, false))
        .collect();
    state.push_discover(options);
}

fn choose_offensive(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    if let Some(power) = power_mut_for(&mut state.auras, POWER) {
        power.counter += 1;
    }
}

fn choose_defensive(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    if let Some(power) = power_mut_for(&mut state.auras, POWER) {
        power.stacks += 1;
    }
}

fn choose_united(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    if let Some(power) = power_mut_for(&mut state.auras, POWER) {
        power.armed += 1;
    }
}
