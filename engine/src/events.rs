//! Structured combat event log (`docs/combat.md` §6).

use crate::model::{BattleOutcome, DeityKind, Side, UnitId};

/// One ordered event in a single-battle replay log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    BattleStart {
        seed: u64,
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
