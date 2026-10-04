//! Public simulation entry points: [`simulate`] and [`simulate_batch`].

use serde::Serialize;

use crate::combat::{resolve_battle, BattleResult};
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
    let mut seeder = Rng::new(base_seed);

    let mut a_wins = 0u32;
    let mut b_wins = 0u32;
    let mut draws = 0u32;
    let mut dmg_min = i32::MAX;
    let mut dmg_max = i32::MIN;
    let mut dmg_sum: i64 = 0;

    for _ in 0..n {
        let battle_seed = seeder.next_u64();
        let res = resolve_battle(board_a, board_b, state, battle_seed);
        let signed_dmg = match res.outcome {
            BattleOutcome::AWin => {
                a_wins += 1;
                res.hero_damage as i32
            }
            BattleOutcome::BWin => {
                b_wins += 1;
                -(res.hero_damage as i32)
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
