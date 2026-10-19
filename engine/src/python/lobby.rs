//! Python bindings for full lobbies ([`crate::lobby_game`], `docs/lobby_game.md`): marshalling
//! only.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::lobby_game::{LobbyConfig, LobbyContext, LobbyGame, OpponentInfo, Phase};
use crate::replay::{self, Timeline};

use super::replay::{optional_note, PyTimeline};
use super::{parse_deity_kind, PyDuosObservation, PyTavernAction, PyTavernObservation, PyUnit};

/// A full lobby: 8 Solo players, or Duos teams of 2, played phase by phase from the hero choice to
/// the last unit standing (`engine/docs/lobby_game.md`). Every engine call is recorded, so
/// `save(path)` writes a replay (`match_replay tui path`).
///
/// Each phase: `begin_phase()`, then every player in `active_players()` acts with `step(p, action)`
/// until it returns `True` (or `force_end(p)` ends their phase for them), then `end_phase()`, which
/// returns the units placed. Units are players in Solo, teams in Duos (players `2t`, `2t + 1`).
#[pyclass(name = "LobbyGame")]
#[derive(Clone)]
pub struct PyLobbyGame {
    inner: LobbyGame,
}

impl PyLobbyGame {
    fn check_player(&self, p: usize) -> PyResult<()> {
        if p < self.inner.num_players() {
            Ok(())
        } else {
            Err(PyValueError::new_err(format!(
                "player {p} does not exist (the lobby has {} players)",
                self.inner.num_players()
            )))
        }
    }

    fn check_unit(&self, u: usize) -> PyResult<()> {
        if u < self.inner.num_units() {
            Ok(())
        } else {
            Err(PyValueError::new_err(format!(
                "unit {u} does not exist (the lobby has {} units)",
                self.inner.num_units()
            )))
        }
    }
}

#[pymethods]
impl PyLobbyGame {
    #[new]
    #[pyo3(signature = (
        players=8, duos=false, seed=0, max_turns=30, heroes=true, hero_offers=2, trinkets=true,
        dark_gifts=true, catalog=None, deity=None, first_fighter_switch_chance=0.75,
        avoid_recent=3, names=None, meta=None
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        players: usize,
        duos: bool,
        seed: u64,
        max_turns: u32,
        heroes: bool,
        hero_offers: usize,
        trinkets: bool,
        dark_gifts: bool,
        catalog: Option<String>,
        deity: Option<&str>,
        first_fighter_switch_chance: f64,
        avoid_recent: usize,
        names: Option<Vec<String>>,
        meta: Option<BTreeMap<String, String>>,
    ) -> PyResult<Self> {
        if !(0.0..=1.0).contains(&first_fighter_switch_chance) {
            return Err(PyValueError::new_err(format!(
                "first_fighter_switch_chance is a probability, got {first_fighter_switch_chance}"
            )));
        }
        let config = LobbyConfig {
            duos,
            players,
            max_turns,
            heroes,
            hero_offers,
            trinkets,
            dark_gifts,
            catalog,
            deity: parse_deity_kind(deity)?,
            first_fighter_switch_chance,
            avoid_recent,
            seed,
            names: names.unwrap_or_default(),
            meta: meta.unwrap_or_default(),
        };
        let inner = LobbyGame::new(config).map_err(PyValueError::new_err)?;
        Ok(Self { inner })
    }

    #[getter]
    fn duos(&self) -> bool {
        self.inner.config().duos
    }

    #[getter]
    fn num_players(&self) -> usize {
        self.inner.num_players()
    }

    #[getter]
    fn num_units(&self) -> usize {
        self.inner.num_units()
    }

    #[getter]
    fn seed(&self) -> u64 {
        self.inner.config().seed
    }

    /// `"hero"`, `"turn"` or `"over"`.
    #[getter]
    fn phase(&self) -> &'static str {
        match self.inner.phase() {
            Phase::Hero => "hero",
            Phase::Turn => "turn",
            Phase::Over => "over",
        }
    }

    /// The current turn (0 before Turn 1).
    #[getter]
    fn turn(&self) -> u32 {
        self.inner.turn()
    }

    /// Every unit is placed.
    #[getter]
    fn done(&self) -> bool {
        self.inner.is_over()
    }

    /// Each unit's final place (1 is first), or `None` while it is still in.
    #[getter]
    fn placements(&self) -> Vec<Option<u32>> {
        self.inner.placements().to_vec()
    }

    /// Whether each unit is still in.
    #[getter]
    fn alive(&self) -> Vec<bool> {
        (0..self.inner.num_units())
            .map(|u| self.inner.is_alive(u))
            .collect()
    }

    /// This turn's pairings (units) and the unit fighting the ghost, if any.
    #[getter]
    fn pairs(&self) -> (Vec<(usize, usize)>, Option<usize>) {
        let (pairs, ghost) = self.inner.pairs();
        (pairs.to_vec(), ghost)
    }

    fn unit_of(&self, p: usize) -> PyResult<usize> {
        self.check_player(p)?;
        Ok(self.inner.unit_of(p))
    }

    fn unit_players(&self, u: usize) -> PyResult<Vec<usize>> {
        self.check_unit(u)?;
        Ok(self.inner.unit_players(u))
    }

    /// Unit `u`'s Health and Armor.
    fn unit_health(&self, u: usize) -> PyResult<(i32, i32)> {
        self.check_unit(u)?;
        Ok(self.inner.unit_health(u))
    }

    fn begin_phase(&mut self) -> PyResult<()> {
        self.inner.begin_phase().map_err(PyValueError::new_err)
    }

    /// Resolve the phase (forcing the players still acting to end it); returns the units placed.
    fn end_phase(&mut self) -> PyResult<Vec<usize>> {
        self.inner.end_phase().map_err(PyValueError::new_err)
    }

    /// The players still to act in this phase.
    fn active_players(&self) -> Vec<usize> {
        self.inner.active_players()
    }

    fn is_done(&self, p: usize) -> PyResult<bool> {
        self.check_player(p)?;
        Ok(self.inner.is_done(p))
    }

    /// Player `p`'s observation: a `TavernObservation` in Solo, a `DuosObservation` in Duos.
    fn observe(&self, py: Python<'_>, p: usize) -> PyResult<PyObject> {
        self.check_player(p)?;
        let game = self.inner.game();
        Ok(if game.is_duos() {
            PyDuosObservation::new(&game.teams()[p / 2], p % 2).into_py(py)
        } else {
            PyTavernObservation::from(game.player(p)).into_py(py)
        })
    }

    fn legal_actions(&self, p: usize) -> PyResult<Vec<PyTavernAction>> {
        self.check_player(p)?;
        Ok(self
            .inner
            .valid_actions(p)
            .into_iter()
            .map(|inner| PyTavernAction { inner })
            .collect())
    }

    fn is_legal(&self, p: usize, action: &PyTavernAction) -> PyResult<bool> {
        self.check_player(p)?;
        Ok(self.inner.is_legal(p, &action.inner))
    }

    /// Player `p` takes `action`, recording `note` (a dict: `value`, `logp`, `forced`, ...) with
    /// it. Returns `True` once they are done with this phase.
    #[pyo3(signature = (p, action, note=None))]
    fn step(
        &mut self,
        p: usize,
        action: &PyTavernAction,
        note: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        self.check_player(p)?;
        let note = optional_note(note)?;
        self.inner
            .step(p, action.inner, note)
            .map_err(PyValueError::new_err)
    }

    /// End player `p`'s phase for them: their first hero, or the first option of each pending
    /// prompt, then End Turn (recorded as forced).
    fn force_end(&mut self, p: usize) -> PyResult<()> {
        self.check_player(p)?;
        self.inner.force_end(p).map_err(PyValueError::new_err)
    }

    /// Player `p`'s view of the lobby, as of the start of the turn: the leaderboard and the boards
    /// the next opponents fought with last.
    fn context(&self, p: usize) -> PyResult<PyLobbyContext> {
        self.check_player(p)?;
        Ok(PyLobbyContext::from(self.inner.context(p)))
    }

    /// An independent copy of the lobby (to look ahead).
    fn copy(&self) -> Self {
        self.clone()
    }

    #[getter]
    fn meta(&self) -> BTreeMap<String, String> {
        self.inner.game().header().meta.clone()
    }

    fn set_meta(&mut self, key: String, value: String) {
        self.inner.meta_mut().insert(key, value);
    }

    #[getter]
    fn num_records(&self) -> usize {
        self.inner.game().records().len()
    }

    /// Save the replay of the game so far.
    fn save(&self, path: PathBuf) -> PyResult<()> {
        self.inner
            .game()
            .save(&path)
            .map_err(|e| PyValueError::new_err(format!("{}: {e}", path.display())))
    }

    /// The replay of the game so far, as protobuf bytes.
    fn replay_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &replay::encode(&self.inner.game().replay()))
    }

    /// Re-simulate the game so far.
    fn timeline(&self) -> PyResult<PyTimeline> {
        let inner = Timeline::new(&self.inner.game().replay()).map_err(PyValueError::new_err)?;
        Ok(PyTimeline { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "LobbyGame({}, players={}, phase={}, turn={})",
            if self.duos() { "duos" } else { "solo" },
            self.inner.num_players(),
            self.phase(),
            self.inner.turn()
        )
    }
}

/// A player's view of their lobby (`LobbyGame.context`).
#[pyclass(name = "LobbyContext", get_all)]
#[derive(Clone)]
pub struct PyLobbyContext {
    pub duos: bool,
    pub units: usize,
    pub alive_units: usize,
    /// 1 + the number of living units with more Health plus Armor.
    pub health_rank: usize,
    pub next_is_ghost: bool,
    /// The next opponent first (Duos: the opposing player in this seat, then their teammate),
    /// then the others.
    pub opponents: Vec<PyOpponentInfo>,
}

impl From<LobbyContext> for PyLobbyContext {
    fn from(ctx: LobbyContext) -> Self {
        Self {
            duos: ctx.duos,
            units: ctx.units,
            alive_units: ctx.alive_units,
            health_rank: ctx.health_rank,
            next_is_ghost: ctx.next_is_ghost,
            opponents: ctx
                .opponents
                .into_iter()
                .map(PyOpponentInfo::from)
                .collect(),
        }
    }
}

#[pymethods]
impl PyLobbyContext {
    fn __repr__(&self) -> String {
        format!(
            "LobbyContext(alive_units={}, health_rank={}, opponents={})",
            self.alive_units,
            self.health_rank,
            self.opponents.len()
        )
    }
}

/// One opponent, as a player sees them (`LobbyContext.opponents`).
#[pyclass(name = "OpponentInfo", get_all)]
#[derive(Clone)]
pub struct PyOpponentInfo {
    pub player: usize,
    pub alive: bool,
    pub health: i32,
    pub armor: i32,
    pub tier: u32,
    pub is_next: bool,
    pub is_last: bool,
    pub last_board: Vec<PyUnit>,
}

impl From<OpponentInfo> for PyOpponentInfo {
    fn from(o: OpponentInfo) -> Self {
        Self {
            player: o.player,
            alive: o.alive,
            health: o.health,
            armor: o.armor,
            tier: o.tier,
            is_next: o.is_next,
            is_last: o.is_last,
            last_board: o
                .last_board
                .into_iter()
                .map(|inner| PyUnit { inner })
                .collect(),
        }
    }
}

#[pymethods]
impl PyOpponentInfo {
    fn __repr__(&self) -> String {
        format!(
            "OpponentInfo(player={}, alive={}, health={}+{}, tier={}, next={})",
            self.player, self.alive, self.health, self.armor, self.tier, self.is_next
        )
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyLobbyGame>()?;
    m.add_class::<PyLobbyContext>()?;
    m.add_class::<PyOpponentInfo>()?;
    Ok(())
}
