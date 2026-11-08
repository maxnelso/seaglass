//! Python bindings for full lobbies ([`crate::lobby_game`], `docs/lobby_game.md`): marshalling
//! only.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::cards;
use crate::game::in_lobby;
use crate::lobby_game::{LobbyConfig, LobbyContext, LobbyGame, OpponentInfo, Phase};
use crate::replay::{self, Timeline};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

use super::replay::{optional_note, PyTimeline};
use super::{
    parse_deity_kind, PyDuosObservation, PyDuosTavernGame, PyTavernAction, PyTavernGame,
    PyTavernObservation, PyUnit,
};

fn take_tavern_visible_units(pool: &mut CardPool, state: &TavernState) {
    for u in state
        .board
        .iter()
        .chain(state.hand.iter())
        .chain(state.shop.iter())
        .chain(state.waiting_for_hand.iter())
    {
        pool.take_unit(u);
    }
    if let Some(opts) = &state.discover_pending {
        for u in opts {
            pool.take_unit(u);
        }
    }
    for opts in &state.discover_queue {
        for u in opts {
            pool.take_unit(u);
        }
    }
}

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
        avoid_recent=3, damage_caps=true, tribes=5, names=None, meta=None
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
        damage_caps: bool,
        tribes: usize,
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
            damage_caps,
            tribes,
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

    /// The minion types in the lobby (empty: every type).
    #[getter]
    fn tribes(&self) -> Vec<&'static str> {
        self.inner.tribes().iter().map(|t| t.as_str()).collect()
    }

    /// The most combat damage a unit can take this turn, if the game caps it.
    #[getter]
    fn damage_cap(&self) -> Option<u32> {
        self.inner.game().lobby().damage_cap
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

    /// Build an imperfect-information `TavernGame` for player `p` (Solo or Duos) using only legal
    /// player/team/public observations, a belief-adjusted `CardPool`, and caller-supplied `seed`.
    #[pyo3(signature = (p, seed=0, extra_units=None))]
    fn belief_tavern(
        &self,
        p: usize,
        seed: u64,
        extra_units: Option<Vec<PyUnit>>,
    ) -> PyResult<PyTavernGame> {
        self.check_player(p)?;
        let catalog_name = &self.inner.game().header().catalog;
        let mut templates = cards::catalog_for(catalog_name).map_err(PyValueError::new_err)?;
        let lobby_tribes = self.inner.tribes();
        if !lobby_tribes.is_empty() {
            templates.retain(|t| in_lobby(t.tribe, &lobby_tribes));
        }
        let mut pool = CardPool::new(templates);
        let state = self.inner.player(p).clone();
        if self.duos() {
            let duos_team = &self.inner.game().teams()[p / 2];
            for s in &duos_team.players {
                take_tavern_visible_units(&mut pool, s);
            }
            for q in &duos_team.incoming {
                for u in q {
                    pool.take_unit(u);
                }
            }
        } else {
            take_tavern_visible_units(&mut pool, &state);
        }
        match extra_units {
            Some(units) => {
                for u in &units {
                    pool.take_unit(&u.inner);
                }
            }
            None => {
                let ctx = self.inner.context(p);
                for opp in &ctx.opponents {
                    if opp.alive {
                        for u in &opp.seen_board {
                            pool.take_unit(u);
                        }
                    }
                }
            }
        }
        Ok(PyTavernGame {
            state,
            pool,
            rng: Rng::new(seed),
        })
    }

    /// Build an imperfect-information `DuosTavernGame` for Duos team `team` using only legal
    /// team/public observations, a belief-adjusted `CardPool`, and caller-supplied `seed`.
    #[pyo3(signature = (team, seed=0, extra_units=None))]
    fn belief_duos_tavern(
        &self,
        team: usize,
        seed: u64,
        extra_units: Option<Vec<PyUnit>>,
    ) -> PyResult<PyDuosTavernGame> {
        if !self.duos() {
            return Err(PyValueError::new_err(
                "belief_duos_tavern is for Duos lobbies; use belief_tavern in Solo",
            ));
        }
        self.check_unit(team)?;
        let catalog_name = &self.inner.game().header().catalog;
        let mut templates = cards::catalog_for(catalog_name).map_err(PyValueError::new_err)?;
        let lobby_tribes = self.inner.tribes();
        if !lobby_tribes.is_empty() {
            templates.retain(|t| in_lobby(t.tribe, &lobby_tribes));
        }
        let mut pool = CardPool::new(templates);
        let duos_team = self.inner.game().teams()[team].clone();
        for s in &duos_team.players {
            take_tavern_visible_units(&mut pool, s);
        }
        for q in &duos_team.incoming {
            for u in q {
                pool.take_unit(u);
            }
        }
        match extra_units {
            Some(units) => {
                for u in &units {
                    pool.take_unit(&u.inner);
                }
            }
            None => {
                let ctx = self.inner.context(2 * team);
                for opp in &ctx.opponents {
                    if opp.alive {
                        for u in &opp.seen_board {
                            pool.take_unit(u);
                        }
                    }
                }
            }
        }
        Ok(PyDuosTavernGame {
            team: duos_team,
            pool,
            rng: Rng::new(seed),
        })
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
    /// The most combat damage the player's unit can take this turn, if capped.
    pub damage_cap: Option<u32>,
    /// The minion types in the lobby (empty: every type).
    pub tribes: Vec<&'static str>,
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
            damage_cap: ctx.damage_cap,
            tribes: ctx.tribes.iter().map(|t| t.as_str()).collect(),
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

/// One opponent, as a player sees them (`LobbyContext.opponents`): the leaderboard, and the board
/// they had when this player last fought them (`seen_board`, from turn `seen_turn`; empty and
/// `None` if never).
#[pyclass(name = "OpponentInfo", get_all)]
#[derive(Clone)]
pub struct PyOpponentInfo {
    pub player: usize,
    pub alive: bool,
    pub health: i32,
    pub armor: i32,
    pub tier: u32,
    pub hero_id: u32,
    pub win_streak: u32,
    pub is_next: bool,
    pub is_last: bool,
    pub seen_board: Vec<PyUnit>,
    pub seen_turn: Option<u32>,
}

impl From<OpponentInfo> for PyOpponentInfo {
    fn from(o: OpponentInfo) -> Self {
        Self {
            player: o.player,
            alive: o.alive,
            health: o.health,
            armor: o.armor,
            tier: o.tier,
            hero_id: o.hero_id,
            win_streak: o.win_streak,
            is_next: o.is_next,
            is_last: o.is_last,
            seen_board: o
                .seen_board
                .into_iter()
                .map(|inner| PyUnit { inner })
                .collect(),
            seen_turn: o.seen_turn,
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
