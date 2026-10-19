//! Python bindings for recorded matches and replays (`docs/replays.md`): marshalling only.
//!
//! `Match` mirrors [`crate::game::Match`]: `match.player(i)` returns a handle with the
//! `TavernGame` API (`observe`, `legal_actions`, `is_legal`, `step`, `start_turn`, `end_turn`,
//! `resolve_combat_with`), so code written against `TavernGame` runs unchanged on a recorded
//! match. `Timeline` mirrors [`crate::replay::Timeline`].

use std::collections::BTreeMap;
use std::path::PathBuf;

use pyo3::exceptions::{PyIndexError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

use crate::combat::BattleResult;
use crate::game::{Match, MatchSetup, PlayerSetup, PoolMode};
use crate::model::BattleOutcome;
use crate::replay::{self, pb, FrameKind, Timeline};
use crate::tavern::TavernState;

use super::{
    deity_kind_str, parse_deity_kind, parse_hero, PyBattleResult, PyTavernAction,
    PyTavernObservation, PyUnit,
};

impl From<BattleResult> for PyBattleResult {
    fn from(res: BattleResult) -> Self {
        let outcome = match res.outcome {
            BattleOutcome::AWin => "a_win",
            BattleOutcome::BWin => "b_win",
            BattleOutcome::Draw => "draw",
        };
        Self {
            outcome: outcome.to_string(),
            survivors_a: res
                .survivors_a
                .into_iter()
                .map(|inner| PyUnit { inner })
                .collect(),
            survivors_b: res
                .survivors_b
                .into_iter()
                .map(|inner| PyUnit { inner })
                .collect(),
            hero_damage: res.hero_damage,
        }
    }
}

fn player_setup(settings: &Bound<'_, PyDict>) -> PyResult<PlayerSetup> {
    let mut setup = PlayerSetup::default();
    for (key, value) in settings.iter() {
        let key: String = key.extract()?;
        match key.as_str() {
            "name" => setup.name = value.extract()?,
            "seed" => setup.seed = value.extract()?,
            "deity" => {
                let deity: Option<String> = value.extract()?;
                setup.deity = parse_deity_kind(deity.as_deref())?;
            }
            "armor" => setup.armor = value.extract()?,
            "shop_spells" => setup.shop_spells = value.extract()?,
            "hero" => setup.hero = parse_hero(Some(&value))?,
            "trinkets" => setup.trinkets = value.extract()?,
            "dark_gifts" => setup.dark_gifts = value.extract()?,
            other => {
                return Err(PyValueError::new_err(format!(
                    "unknown player setting {other:?} (expected name, seed, deity, armor, \
                     shop_spells, hero, trinkets or dark_gifts)"
                )))
            }
        }
    }
    Ok(setup)
}

/// A note from a dict: `value`, `logp`, `forced`, `candidates` (a list of `(TavernAction,
/// probability)` pairs); any other key must be a number and goes into `extra`.
fn parse_note(note: &Bound<'_, PyAny>) -> PyResult<pb::Note> {
    let note = note
        .downcast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("a note must be a dict"))?;
    let mut parsed = pb::Note::default();
    for (key, value) in note.iter() {
        let key: String = key.extract()?;
        match key.as_str() {
            "value" => parsed.value = note_entry(&key, &value, "a number or None")?,
            "logp" => parsed.logp = note_entry(&key, &value, "a number or None")?,
            "forced" => parsed.forced = note_entry(&key, &value, "a bool")?,
            "candidates" => {
                let candidates: Vec<(PyTavernAction, f32)> =
                    note_entry(&key, &value, "a list of (TavernAction, probability) pairs")?;
                parsed.candidates = candidates
                    .into_iter()
                    .map(|(action, prob)| pb::Candidate {
                        action: Some(replay::action_to_pb(&action.inner)),
                        prob,
                    })
                    .collect();
            }
            _ => {
                let number: f64 = note_entry(&key, &value, "a number")?;
                parsed.extra.insert(key, number);
            }
        }
    }
    Ok(parsed)
}

fn note_entry<'py, T: FromPyObject<'py>>(
    key: &str,
    value: &Bound<'py, PyAny>,
    expected: &str,
) -> PyResult<T> {
    value
        .extract()
        .map_err(|_| PyTypeError::new_err(format!("note entry {key:?} must be {expected}")))
}

/// The dict form of a note (the same shape `parse_note` reads).
fn note_to_py(py: Python<'_>, note: &pb::Note) -> PyResult<PyObject> {
    let dict = PyDict::new_bound(py);
    if let Some(value) = note.value {
        dict.set_item("value", value)?;
    }
    if let Some(logp) = note.logp {
        dict.set_item("logp", logp)?;
    }
    dict.set_item("forced", note.forced)?;
    if !note.candidates.is_empty() {
        let candidates = note
            .candidates
            .iter()
            .map(|c| {
                let action = c.action.as_ref().ok_or("a candidate has no action")?;
                let inner = replay::action_from_pb(action)?;
                Ok((PyTavernAction { inner }, c.prob))
            })
            .collect::<Result<Vec<_>, String>>()
            .map_err(PyValueError::new_err)?;
        dict.set_item("candidates", candidates.into_py(py))?;
    }
    for (key, value) in &note.extra {
        dict.set_item(key, value)?;
    }
    Ok(dict.into_any().unbind())
}

fn optional_note(note: Option<&Bound<'_, PyAny>>) -> PyResult<Option<pb::Note>> {
    note.filter(|n| !n.is_none()).map(parse_note).transpose()
}

/// A match whose every engine call is recorded into a replay (`docs/replays.md`).
///
/// `players` is a list of dicts with the `TavernGame` settings: `name`, `seed`, `deity`
/// (`None`, `"cthun"` or `"yshaarj"`), `armor` (default 5) and `shop_spells` (default `True`).
/// `pool` is `"per_player"` (every player has their own pool and RNG, seeded by their `seed`,
/// like separate `TavernGame`s) or `"shared"` (one pool and RNG seeded by `shared_seed`).
#[pyclass(name = "Match")]
pub struct PyMatch {
    inner: Match,
}

impl PyMatch {
    fn check_player(&self, p: usize) -> PyResult<()> {
        let n = self.inner.num_players();
        if p < n {
            Ok(())
        } else {
            Err(PyIndexError::new_err(format!(
                "player {p} does not exist (the match has {n} players)"
            )))
        }
    }

    fn combat(&mut self, a: usize, b: usize, seed: Option<u64>) -> PyResult<PyBattleResult> {
        self.check_player(a)?;
        self.check_player(b)?;
        if a == b {
            return Err(PyValueError::new_err(format!(
                "player {a} cannot fight themselves"
            )));
        }
        let res = match seed {
            Some(seed) => self.inner.combat_with_seed(a, b, seed),
            None => self.inner.combat(a, b),
        };
        Ok(res.into())
    }
}

#[pymethods]
impl PyMatch {
    #[new]
    #[pyo3(signature = (players, catalog="full", pool="per_player", shared_seed=0, meta=None))]
    fn new(
        players: Vec<Bound<'_, PyDict>>,
        catalog: &str,
        pool: &str,
        shared_seed: u64,
        meta: Option<BTreeMap<String, String>>,
    ) -> PyResult<Self> {
        let pool = match pool {
            "per_player" => PoolMode::PerPlayer,
            "shared" => PoolMode::Shared,
            other => {
                return Err(PyValueError::new_err(format!(
                    "unknown pool mode {other:?} (expected 'per_player' or 'shared')"
                )))
            }
        };
        let setup = MatchSetup {
            catalog: catalog.to_string(),
            pool,
            shared_seed,
            players: players.iter().map(player_setup).collect::<PyResult<_>>()?,
            meta: meta.unwrap_or_default(),
        };
        let inner = Match::new(&setup).map_err(PyValueError::new_err)?;
        Ok(Self { inner })
    }

    #[getter]
    fn num_players(&self) -> usize {
        self.inner.num_players()
    }

    /// A handle on player `index` with the `TavernGame` API.
    fn player(slf: &Bound<'_, Self>, index: usize) -> PyResult<PyMatchPlayer> {
        slf.borrow().check_player(index)?;
        Ok(PyMatchPlayer {
            game: slf.clone().unbind(),
            index,
        })
    }

    /// Handles on every player.
    fn players(slf: &Bound<'_, Self>) -> Vec<PyMatchPlayer> {
        (0..slf.borrow().inner.num_players())
            .map(|index| PyMatchPlayer {
                game: slf.clone().unbind(),
                index,
            })
            .collect()
    }

    /// Players `a` and `b` fight; the seed is drawn from player `a`'s RNG unless given.
    #[pyo3(name = "combat", signature = (a, b, seed=None))]
    fn py_combat(&mut self, a: usize, b: usize, seed: Option<u64>) -> PyResult<PyBattleResult> {
        self.combat(a, b, seed)
    }

    /// The coming turn's pairings (`(a, b)` pairs of players): the lobby records them and every
    /// player gets their view (`Lobby.set_next_pairs` + `Lobby.apply`). Call it before the players
    /// start their turns.
    fn set_pairs(&mut self, pairs: Vec<(usize, usize)>) -> PyResult<()> {
        for &(a, b) in &pairs {
            self.check_player(a)?;
            self.check_player(b)?;
            if a == b {
                return Err(PyValueError::new_err(format!(
                    "player {a} cannot be paired with themselves"
                )));
            }
        }
        self.inner.set_pairs(&pairs);
        Ok(())
    }

    /// Every player's lobby gifts reach their living opponents (`Lobby.deliver_gifts`).
    fn deliver_gifts(&mut self) {
        self.inner.deliver_gifts();
    }

    /// The combats are over: the lobby records who is still alive (above 0 Health) and returns it
    /// (`Lobby.record_deaths`). `combat` already records each combat and its outcome.
    fn record_deaths(&mut self) -> Vec<bool> {
        self.inner.record_deaths()
    }

    /// Record the end of the match. Returns the winner (the player with the most Health), or
    /// `None` for a tie.
    fn finish(&mut self) -> Option<usize> {
        self.inner.finish()
    }

    #[getter]
    fn meta(&self) -> BTreeMap<String, String> {
        self.inner.header().meta.clone()
    }

    fn set_meta(&mut self, key: String, value: String) {
        self.inner.meta_mut().insert(key, value);
    }

    /// The number of calls recorded so far.
    #[getter]
    fn num_records(&self) -> usize {
        self.inner.records().len()
    }

    /// Save the replay of the match so far.
    fn save(&self, path: PathBuf) -> PyResult<()> {
        self.inner
            .save(&path)
            .map_err(|e| PyValueError::new_err(format!("{}: {e}", path.display())))
    }

    /// The replay of the match so far, as protobuf bytes.
    fn replay_bytes<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new_bound(py, &replay::encode(&self.inner.replay()))
    }

    /// Re-simulate the match so far.
    fn timeline(&self) -> PyResult<PyTimeline> {
        let inner = Timeline::new(&self.inner.replay()).map_err(PyValueError::new_err)?;
        Ok(PyTimeline { inner })
    }

    fn __repr__(&self) -> String {
        let turn = self
            .inner
            .players()
            .iter()
            .map(|p| p.turn)
            .max()
            .unwrap_or(0);
        format!(
            "Match(players={}, turn={turn}, records={})",
            self.inner.num_players(),
            self.inner.records().len()
        )
    }
}

/// One player of a `Match`, with the `TavernGame` API. Every call is recorded.
#[pyclass(name = "MatchPlayer")]
pub struct PyMatchPlayer {
    game: Py<PyMatch>,
    index: usize,
}

#[pymethods]
impl PyMatchPlayer {
    #[getter]
    fn index(&self) -> usize {
        self.index
    }

    #[getter]
    fn name(&self, py: Python<'_>) -> String {
        self.game.borrow(py).inner.header().players[self.index]
            .name
            .clone()
    }

    #[getter]
    fn game(&self, py: Python<'_>) -> Py<PyMatch> {
        self.game.clone_ref(py)
    }

    fn state(&self, py: Python<'_>) -> PyTavernObservation {
        PyTavernObservation::from(self.game.borrow(py).inner.player(self.index))
    }

    fn observe(&self, py: Python<'_>) -> PyTavernObservation {
        self.state(py)
    }

    fn start_turn(&self, py: Python<'_>) {
        self.game.borrow_mut(py).inner.start_turn(self.index);
    }

    #[pyo3(signature = (note=None))]
    fn end_turn(&self, py: Python<'_>, note: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let note = optional_note(note)?;
        self.game
            .borrow_mut(py)
            .inner
            .step_with_note(self.index, crate::tavern::TavernAction::EndTurn, note)
            .map(|_| ())
            .map_err(PyValueError::new_err)
    }

    fn is_legal(&self, py: Python<'_>, action: &PyTavernAction) -> bool {
        self.game
            .borrow(py)
            .inner
            .player(self.index)
            .is_legal(&action.inner)
    }

    fn legal_actions(&self, py: Python<'_>) -> Vec<PyTavernAction> {
        legal_actions(self.game.borrow(py).inner.player(self.index))
    }

    /// Take `action`, recording `note` (a dict, see `Match`) with it. Returns whether the turn
    /// ended; raises `ValueError` (and records nothing) if the action is illegal.
    #[pyo3(signature = (action, note=None))]
    fn step(
        &self,
        py: Python<'_>,
        action: &PyTavernAction,
        note: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let note = optional_note(note)?;
        self.game
            .borrow_mut(py)
            .inner
            .step_with_note(self.index, action.inner, note)
            .map_err(PyValueError::new_err)
    }

    /// Fight `other` (a player of the same match); the seed is drawn from this player's RNG
    /// unless given, like `TavernGame.resolve_combat_with`.
    #[pyo3(signature = (other, seed=None))]
    fn resolve_combat_with(
        &self,
        py: Python<'_>,
        other: &PyMatchPlayer,
        seed: Option<u64>,
    ) -> PyResult<PyBattleResult> {
        if self.game.as_ptr() != other.game.as_ptr() {
            return Err(PyValueError::new_err(
                "both players must belong to the same Match",
            ));
        }
        self.game
            .borrow_mut(py)
            .combat(self.index, other.index, seed)
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        let game = self.game.borrow(py);
        let state = game.inner.player(self.index);
        format!(
            "MatchPlayer(index={}, name={:?}, turn={}, health={})",
            self.index,
            game.inner.header().players[self.index].name,
            state.turn,
            state.health
        )
    }
}

fn legal_actions(state: &TavernState) -> Vec<PyTavernAction> {
    state
        .valid_actions()
        .into_iter()
        .map(|inner| PyTavernAction { inner })
        .collect()
}

/// `(frame, player, action, note)`: one action of a `Timeline`.
type Decision = (usize, usize, PyTavernAction, Option<PyObject>);

/// A replay re-simulated into the state after every call (`docs/replays.md`).
#[pyclass(name = "Timeline")]
pub struct PyTimeline {
    inner: Timeline,
}

impl PyTimeline {
    fn frame_index(&self, index: isize) -> PyResult<usize> {
        let len = self.inner.len() as isize;
        let i = if index < 0 { index + len } else { index };
        if (0..len).contains(&i) {
            Ok(i as usize)
        } else {
            Err(PyIndexError::new_err(format!(
                "frame {index} does not exist (the timeline has {len} frames)"
            )))
        }
    }
}

#[pymethods]
impl PyTimeline {
    /// Load and re-simulate a replay file.
    #[staticmethod]
    fn load(path: PathBuf) -> PyResult<Self> {
        let inner = Timeline::load(&path).map_err(PyValueError::new_err)?;
        Ok(Self { inner })
    }

    /// Re-simulate a replay from its protobuf bytes.
    #[staticmethod]
    fn from_bytes(data: &[u8]) -> PyResult<Self> {
        let replay = replay::decode(data).map_err(PyValueError::new_err)?;
        let inner = Timeline::new(&replay).map_err(PyValueError::new_err)?;
        Ok(Self { inner })
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __getitem__(&self, index: isize) -> PyResult<PyFrame> {
        let i = self.frame_index(index)?;
        Ok(PyFrame::new(&self.inner, i))
    }

    /// The match setup: `format`, `engine`, `catalog`, `pool`, `shared_seed`, `players` (dicts)
    /// and `meta`.
    #[getter]
    fn header(&self, py: Python<'_>) -> PyResult<PyObject> {
        let header = &self.inner.header;
        let dict = PyDict::new_bound(py);
        dict.set_item("format", header.format)?;
        dict.set_item("engine", &header.engine)?;
        dict.set_item("catalog", &header.catalog)?;
        let pool = match header.pool() {
            pb::PoolMode::Shared => "shared",
            _ => "per_player",
        };
        dict.set_item("pool", pool)?;
        dict.set_item("shared_seed", header.shared_seed)?;
        let players = header
            .players
            .iter()
            .map(|p| {
                let player = PyDict::new_bound(py);
                player.set_item("name", &p.name)?;
                player.set_item("seed", p.seed)?;
                let deity = replay::deity_from_pb(p.deity());
                player.set_item("deity", deity_kind_str(deity))?;
                player.set_item("armor", p.armor)?;
                player.set_item("shop_spells", p.shop_spells)?;
                // The hero's name (None: no hero), as `TavernGame(hero=...)` takes it.
                let hero = p
                    .hero
                    .map(|id| crate::heroes::data(id).map_or("?", |h| h.name));
                player.set_item("hero", hero)?;
                player.set_item("trinkets", p.trinkets)?;
                player.set_item("dark_gifts", p.dark_gifts)?;
                Ok(player.into_any().unbind())
            })
            .collect::<PyResult<Vec<PyObject>>>()?;
        dict.set_item("players", players)?;
        dict.set_item("meta", header.meta.clone())?;
        Ok(dict.into_any().unbind())
    }

    #[getter]
    fn num_players(&self) -> usize {
        self.inner.num_players()
    }

    /// The highest turn reached.
    #[getter]
    fn last_turn(&self) -> u32 {
        self.inner.last_turn()
    }

    /// `(record, frame, message)` for every call whose re-simulated result differs.
    #[getter]
    fn divergences(&self) -> Vec<(usize, usize, String)> {
        self.inner
            .divergences
            .iter()
            .map(|d| (d.record, d.frame, d.message.clone()))
            .collect()
    }

    /// `(record, message)` for the call re-simulation stopped at, if any.
    #[getter]
    fn error(&self) -> Option<(usize, String)> {
        self.inner
            .error
            .as_ref()
            .map(|e| (e.record, e.message.clone()))
    }

    #[getter]
    fn is_faithful(&self) -> bool {
        self.inner.is_faithful()
    }

    /// The engine that recorded the replay, if it is not this build.
    #[getter]
    fn engine_mismatch(&self) -> Option<String> {
        self.inner.engine_mismatch().map(str::to_string)
    }

    /// The index of the first frame on turn `turn` or later.
    fn turn_start(&self, turn: u32) -> Option<usize> {
        self.inner.turn_start(turn)
    }

    /// `(frame, player, action, note)` for every action, in order. The state the action was
    /// taken in is `player_at(frame - 1, player)`.
    fn decisions(&self, py: Python<'_>) -> PyResult<Vec<Decision>> {
        self.inner
            .decisions()
            .map(|d| {
                let note = d.note.map(|n| note_to_py(py, n)).transpose()?;
                Ok((d.frame, d.player, PyTavernAction { inner: d.action }, note))
            })
            .collect()
    }

    /// A read-only handle on player `player` at frame `frame`, with the observing part of the
    /// `TavernGame` API (`observe`, `legal_actions`, `is_legal`), e.g. for `action_mask`.
    fn player_at(slf: &Bound<'_, Self>, frame: isize, player: usize) -> PyResult<PyFramePlayer> {
        let timeline = slf.borrow();
        let frame = timeline.frame_index(frame)?;
        let n = timeline.inner.num_players();
        if player >= n {
            return Err(PyIndexError::new_err(format!(
                "player {player} does not exist (the match has {n} players)"
            )));
        }
        Ok(PyFramePlayer {
            timeline: slf.clone().unbind(),
            frame,
            index: player,
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "Timeline(frames={}, turns={}, faithful={})",
            self.inner.len(),
            self.inner.last_turn(),
            if self.inner.is_faithful() {
                "True"
            } else {
                "False"
            }
        )
    }
}

/// The match after one call of a `Timeline`.
#[pyclass(name = "Frame")]
pub struct PyFrame {
    #[pyo3(get)]
    index: usize,
    /// `"start"`, `"start_turn"`, `"act"`, `"combat"`, `"set_pairs"`, `"deliver_gifts"`,
    /// `"record_deaths"` or `"end"`.
    #[pyo3(get)]
    kind: &'static str,
    /// The replay record this frame re-simulates (`None` for the start).
    #[pyo3(get)]
    record: Option<usize>,
    /// The highest turn any player has reached.
    #[pyo3(get)]
    turn: u32,
    /// The player who started their turn or acted.
    #[pyo3(get)]
    player: Option<usize>,
    #[pyo3(get)]
    action: Option<PyTavernAction>,
    /// The card the action used.
    #[pyo3(get)]
    card_id: Option<u32>,
    note: Option<pb::Note>,
    /// The two players of a combat.
    #[pyo3(get)]
    combatants: Option<(usize, usize)>,
    #[pyo3(get)]
    seed: Option<u64>,
    #[pyo3(get)]
    battle: Option<PyBattleResult>,
    /// The number of events in a combat's log.
    #[pyo3(get)]
    num_events: Option<usize>,
    #[pyo3(get)]
    winner: Option<usize>,
    /// Every player's state after the call.
    #[pyo3(get)]
    players: Vec<PyTavernObservation>,
    #[pyo3(get)]
    rng_states: Vec<u64>,
}

impl PyFrame {
    fn new(timeline: &Timeline, index: usize) -> Self {
        let frame = &timeline.frames[index];
        let mut out = Self {
            index,
            kind: "start",
            record: frame.record,
            turn: frame.turn,
            player: None,
            action: None,
            card_id: None,
            note: None,
            combatants: None,
            seed: None,
            battle: None,
            num_events: None,
            winner: None,
            players: frame
                .players
                .iter()
                .map(PyTavernObservation::from)
                .collect(),
            rng_states: frame.rng_states.clone(),
        };
        match &frame.kind {
            FrameKind::Start => {}
            FrameKind::StartTurn { player, .. } => {
                out.kind = "start_turn";
                out.player = Some(*player);
            }
            FrameKind::Act {
                player,
                action,
                card_id,
                note,
            } => {
                out.kind = "act";
                out.player = Some(*player);
                out.action = Some(PyTavernAction { inner: *action });
                out.card_id = *card_id;
                out.note = note.clone();
            }
            FrameKind::Combat { a, b, seed, result } => {
                out.kind = "combat";
                out.combatants = Some((*a, *b));
                out.seed = Some(*seed);
                out.num_events = Some(result.events.len());
                out.battle = Some(PyBattleResult::from(result.as_ref().clone()));
            }
            FrameKind::SetPairs { .. } => out.kind = "set_pairs",
            FrameKind::DeliverGifts => out.kind = "deliver_gifts",
            FrameKind::RecordDeaths { .. } => out.kind = "record_deaths",
            FrameKind::End { winner } => {
                out.kind = "end";
                out.winner = *winner;
            }
        }
        out
    }
}

#[pymethods]
impl PyFrame {
    /// What the actor noted about the action (a dict, see `Match`), if anything.
    #[getter]
    fn note(&self, py: Python<'_>) -> PyResult<Option<PyObject>> {
        self.note.as_ref().map(|n| note_to_py(py, n)).transpose()
    }

    fn __repr__(&self) -> String {
        let player = self.player.map_or("None".to_string(), |p| p.to_string());
        let action = self
            .action
            .map_or("None".to_string(), |a| format!("{:?}", a.inner));
        format!(
            "Frame(index={}, kind={:?}, turn={}, player={player}, action={action})",
            self.index, self.kind, self.turn
        )
    }
}

/// One player at one frame of a `Timeline`, with the observing part of the `TavernGame` API.
#[pyclass(name = "FramePlayer")]
pub struct PyFramePlayer {
    timeline: Py<PyTimeline>,
    frame: usize,
    index: usize,
}

impl PyFramePlayer {
    fn with_state<T>(&self, py: Python<'_>, f: impl FnOnce(&TavernState) -> T) -> T {
        let timeline = self.timeline.borrow(py);
        f(&timeline.inner.frames[self.frame].players[self.index])
    }
}

#[pymethods]
impl PyFramePlayer {
    #[getter]
    fn index(&self) -> usize {
        self.index
    }

    #[getter]
    fn frame(&self) -> usize {
        self.frame
    }

    fn state(&self, py: Python<'_>) -> PyTavernObservation {
        self.with_state(py, |state| PyTavernObservation::from(state))
    }

    fn observe(&self, py: Python<'_>) -> PyTavernObservation {
        self.state(py)
    }

    fn is_legal(&self, py: Python<'_>, action: &PyTavernAction) -> bool {
        self.with_state(py, |s| s.is_legal(&action.inner))
    }

    fn legal_actions(&self, py: Python<'_>) -> Vec<PyTavernAction> {
        self.with_state(py, legal_actions)
    }

    fn __repr__(&self) -> String {
        format!("FramePlayer(frame={}, index={})", self.frame, self.index)
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyMatch>()?;
    m.add_class::<PyMatchPlayer>()?;
    m.add_class::<PyTimeline>()?;
    m.add_class::<PyFrame>()?;
    m.add_class::<PyFramePlayer>()?;
    m.add("ENGINE_VERSION", replay::engine_version())?;
    Ok(())
}
