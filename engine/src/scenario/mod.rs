//! YAML scenarios: the engine's test format (`docs/scenarios.md`, `tests/scenarios/`).
//!
//! A scenario file holds scenarios for one card (`card:`) or one topic (`topic:`). Each
//! scenario sets up a Tavern (or a standalone combat), runs steps through the public engine API
//! (`TavernState::step`, `add_to_hand`, `resolve_combat_against`, `simulate`, ...), and checks
//! expectations against a serde view of the resulting state. Nothing here knows about specific
//! cards: units are named by card name, and expectations are generic structural matches.
//!
//! - [`ScenarioFile::load`] parses and validates a file.
//! - [`Scenario::run`] runs one scenario (twice per seed: the runs must be identical).
//!
//! The board-vs-board matchup format used by `combat_cli` ([`Matchup`]) and the unit specs it
//! uses ([`parse_unit`], also exposed to Python) live here too.

mod matcher;
mod matchup;
mod patch;
mod runner;
mod spec;
mod units;
mod view;

use std::fmt;

pub use matchup::{parse_unit, teams_and_state, Batch, Defaults, Matchup};
pub use spec::{Header, Scenario, ScenarioFile};
pub use units::{card_index, slug, Card, CardIndex, CardKind};

/// Why a scenario failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The scenario itself is wrong: an unknown card, field, step or operator, a malformed
    /// value, ... (never counts as reproducing a `known_bug`).
    Invalid(String),
    /// The engine did not behave as the scenario expects (an expectation failed, or the engine
    /// rejected an action the scenario performs).
    Mismatch(String),
}

impl Failure {
    /// The failure message.
    pub fn message(&self) -> &str {
        match self {
            Failure::Invalid(m) | Failure::Mismatch(m) => m,
        }
    }

    /// The same failure with `prefix` prepended to its message.
    pub(crate) fn prefixed(self, prefix: &str) -> Self {
        match self {
            Failure::Invalid(m) => Failure::Invalid(format!("{prefix}{m}")),
            Failure::Mismatch(m) => Failure::Mismatch(format!("{prefix}{m}")),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::Invalid(m) => write!(f, "invalid scenario: {m}"),
            Failure::Mismatch(m) => f.write_str(m),
        }
    }
}
