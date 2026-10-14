//! Structured combat event log (`docs/combat.md` §6, `docs/duos.md` §7).

use serde::Serialize;

use crate::model::{BattleOutcome, DeityKind, Side, UnitId};

/// One ordered event in a single-battle replay log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    BattleStart {
        seed: u64,
        first_attacker: Side,
    },
    /// A Duos warband enters the fight: one of the two opening warbands, or a warband stepping
    /// in for its teammate (`docs/duos.md` §7). `player` is its index in its team (0 or 1).
    WarbandSteppedIn {
        team: Side,
        player: usize,
        first_attacker: Side,
    },
    AttackDeclared {
        side: Side,
        attacker: UnitId,
        target: UnitId,
    },
    StatBuff {
        side: Side,
        unit: UnitId,
        atk_delta: i32,
        hp_delta: i32,
        attack: i32,
        health: i32,
        reason: &'static str,
    },
    UnitSummoned {
        side: Side,
        source: UnitId,
        unit: UnitId,
        name: String,
        attack: i32,
        health: i32,
        reason: &'static str,
    },
    /// A minion transformed into a copy of another card, keeping its id and place (Duos
    /// `Sandy`): its new name and stats.
    UnitTransformed {
        side: Side,
        unit: UnitId,
        name: String,
        attack: i32,
        health: i32,
    },
    DamageDealt {
        unit: UnitId,
        amount: i32,
        from: UnitId,
    },
    DivineShieldPopped {
        unit: UnitId,
    },
    VenomousTriggered {
        attacker: UnitId,
        target: UnitId,
    },
    Death {
        unit: UnitId,
    },
    DeityAwakened {
        side: Side,
        deity: DeityKind,
        unit: UnitId,
        name: String,
    },
    TurnSkipped {
        side: Side,
    },
    BattleEnd {
        outcome: BattleOutcome,
        hero_damage: u32,
    },
}
