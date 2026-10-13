//! Public simulation entry points: [`simulate`] and [`simulate_batch`], and their Duos
//! counterparts [`simulate_duos`] and [`simulate_duos_batch`].

use serde::Serialize;

use crate::combat::{resolve_battle, resolve_duos_battle, BattleResult, DuosResult, DuosTeam};
use crate::model::{BattleOutcome, GameState, Unit};
use crate::rng::Rng;

/// Summary statistics for hero damage across a batch of battles.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct DamageStats {
    pub min: i32,
    pub max: i32,
    pub mean: f64,
}

/// Aggregated outcome distribution across `battles` simulated fights.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BattleDistribution {
    pub battles: u32,
    pub a_wins: u32,
    pub b_wins: u32,
    pub draws: u32,
    pub a_win_rate: f64,
    pub b_win_rate: f64,
    pub draw_rate: f64,
    /// Signed damage from Side A's perspective (`+hero_damage` on A win, `-hero_damage` on B win, `0` on draw).
    pub damage: DamageStats,
}

/// Simulate one deterministic battle from `(board_a, board_b, state, seed)`.
pub fn simulate(
    board_a: &[Unit],
    board_b: &[Unit],
    state: &GameState,
    seed: u64,
) -> BattleResult {
    resolve_battle(board_a, board_b, state, seed)
}

/// Simulate `n` battles from a deterministic seed sequence derived from `base_seed` (`docs/combat.md` §4).
pub fn simulate_batch(
    board_a: &[Unit],
    board_b: &[Unit],
    state: &GameState,
    base_seed: u64,
    n: u32,
) -> BattleDistribution {
    assert!(n > 0, "simulate_batch requires n > 0");
    distribution(base_seed, n, |seed| {
        let res = resolve_battle(board_a, board_b, state, seed);
        (res.outcome, res.hero_damage)
    })
}

/// Simulate one deterministic Duos combat from `(team_a, team_b, seed)` (`docs/duos.md`).
pub fn simulate_duos(team_a: &DuosTeam, team_b: &DuosTeam, seed: u64) -> DuosResult {
    resolve_duos_battle(team_a, team_b, seed)
}

/// Simulate `n` Duos combats from a deterministic seed sequence derived from `base_seed`, as
/// [`simulate_batch`] does (`docs/duos.md` §5). Team A is side A.
pub fn simulate_duos_batch(
    team_a: &DuosTeam,
    team_b: &DuosTeam,
    base_seed: u64,
    n: u32,
) -> BattleDistribution {
    assert!(n > 0, "simulate_duos_batch requires n > 0");
    distribution(base_seed, n, |seed| {
        let res = resolve_duos_battle(team_a, team_b, seed);
        (res.outcome, res.hero_damage)
    })
}

/// The distribution of `n` battles: battle `i` (0-indexed) is `battle` run with the `(i + 1)`-th
/// `next_u64()` of a `SplitMix64` seeded with `base_seed`, and gives its outcome and hero damage
/// (`docs/combat.md` §4).
fn distribution(
    base_seed: u64,
    n: u32,
    mut battle: impl FnMut(u64) -> (BattleOutcome, u32),
) -> BattleDistribution {
    let mut seeder = Rng::new(base_seed);

    let mut a_wins = 0u32;
    let mut b_wins = 0u32;
    let mut draws = 0u32;
    let mut dmg_min = i32::MAX;
    let mut dmg_max = i32::MIN;
    let mut dmg_sum: i64 = 0;

    for _ in 0..n {
        let battle_seed = seeder.next_u64();
        let (outcome, hero_damage) = battle(battle_seed);
        let signed_dmg = match outcome {
            BattleOutcome::AWin => {
                a_wins += 1;
                hero_damage as i32
            }
            BattleOutcome::BWin => {
                b_wins += 1;
                -(hero_damage as i32)
            }
            BattleOutcome::Draw => {
                draws += 1;
                0
            }
        };
        dmg_min = dmg_min.min(signed_dmg);
        dmg_max = dmg_max.max(signed_dmg);
        dmg_sum += signed_dmg as i64;
    }

    let total = n as f64;
    BattleDistribution {
        battles: n,
        a_wins,
        b_wins,
        draws,
        a_win_rate: (a_wins as f64) / total,
        b_win_rate: (b_wins as f64) / total,
        draw_rate: (draws as f64) / total,
        damage: DamageStats {
            min: dmg_min,
            max: dmg_max,
            mean: (dmg_sum as f64) / total,
        },
    }
}
