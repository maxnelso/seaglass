//! Thin marshalling-only PyO3 bindings (`--features python`).
//!
//! All RL-specific decisions (action flattening, reward shaping, observation tensors,
//! and synthetic opponents) belong in `rl/` rather than here.

#![allow(clippy::useless_conversion)]

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::cards::{self, CardTemplate};
use crate::model::{BattleOutcome, DeityKind, GameState, Keyword, Unit};
use crate::rng::Rng;
use crate::scenario::{parse_unit as rust_parse_unit, Defaults};
use crate::sim::{simulate as rust_simulate, simulate_batch as rust_simulate_batch};
use crate::tavern::{CardPool, TavernAction, TavernState};

fn parse_deity_kind(name: Option<&str>) -> PyResult<DeityKind> {
    match name {
        None | Some("none") | Some("None") => Ok(DeityKind::None),
        Some("cthun") | Some("c'thun") | Some("C'Thun") => Ok(DeityKind::CThun),
        Some("yshaarj") | Some("y'shaarj") | Some("Y'Shaarj") => Ok(DeityKind::YShaarj),
        Some(other) => Err(PyValueError::new_err(format!(
            "unknown deity {other:?} (expected None, 'cthun', or 'yshaarj')"
        ))),
    }
}

fn deity_kind_str(kind: DeityKind) -> &'static str {
    match kind {
        DeityKind::None => "none",
        DeityKind::CThun => "cthun",
        DeityKind::YShaarj => "yshaarj",
    }
}

/// Python wrapper around [`Unit`].
#[pyclass(name = "Unit")]
#[derive(Clone, Debug)]
pub struct PyUnit {
    pub inner: Unit,
}

#[pymethods]
impl PyUnit {
    #[new]
    #[pyo3(signature = (name, attack, health, tavern_tier=1, card_id=0, is_golden=false))]
    fn new(
        name: String,
        attack: i32,
        health: i32,
        tavern_tier: u32,
        card_id: u32,
        is_golden: bool,
    ) -> Self {
        Self {
            inner: Unit::new(name, attack, health)
                .with_tavern_tier(tavern_tier)
                .with_card_id(card_id)
                .with_golden(is_golden),
        }
    }

    #[staticmethod]
    fn from_spec(spec: &str) -> PyResult<Self> {
        let unit =
            rust_parse_unit(spec, &Defaults::default()).map_err(PyValueError::new_err)?;
        Ok(Self { inner: unit })
    }

    #[getter]
    fn card_id(&self) -> u32 {
        self.inner.card_id
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn attack(&self) -> i32 {
        self.inner.attack
    }

    #[getter]
    fn health(&self) -> i32 {
        self.inner.health
    }

    #[getter]
    fn base_attack(&self) -> i32 {
        self.inner.base_attack
    }

    #[getter]
    fn base_health(&self) -> i32 {
        self.inner.base_health
    }

    #[getter]
    fn tavern_tier(&self) -> u32 {
        self.inner.tavern_tier
    }

    #[getter]
    fn tribe(&self) -> &str {
        self.inner.tribe.as_str()
    }

    #[getter]
    fn is_golden(&self) -> bool {
        self.inner.is_golden
    }

    #[getter]
    fn is_spell(&self) -> bool {
        self.inner.is_spell
    }

    #[getter]
    fn taunt(&self) -> bool {
        self.inner.taunt
    }

    #[getter]
    fn divine_shield(&self) -> bool {
        self.inner.divine_shield
    }

    #[getter]
    fn windfury(&self) -> bool {
        self.inner.windfury
    }

    #[getter]
    fn reborn(&self) -> bool {
        self.inner.reborn
    }

    #[getter]
    fn venomous(&self) -> bool {
        self.inner.venomous
    }

    #[getter]
    fn stealth(&self) -> bool {
        self.inner.stealth
    }

    #[getter]
    fn magnetic(&self) -> bool {
        self.inner.magnetic
    }

    #[getter]
    fn activated_this_turn(&self) -> bool {
        self.inner.activated_this_turn
    }

    fn __repr__(&self) -> String {
        format!(
            "Unit(name={:?}, stats={}/{}, tier={}, tribe={:?}, golden={})",
            self.inner.name,
            self.inner.attack,
            self.inner.health,
            self.inner.tavern_tier,
            self.inner.tribe.as_str(),
            self.inner.is_golden,
        )
    }
}

/// Python wrapper around [`CardTemplate`].
#[pyclass(name = "CardTemplate")]
#[derive(Clone, Debug)]
pub struct PyCardTemplate {
    pub inner: CardTemplate,
}

#[pymethods]
impl PyCardTemplate {
    #[getter]
    fn card_id(&self) -> u32 {
        self.inner.card_id
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn attack(&self) -> i32 {
        self.inner.attack
    }

    #[getter]
    fn health(&self) -> i32 {
        self.inner.health
    }

    #[getter]
    fn tavern_tier(&self) -> u32 {
        self.inner.tavern_tier
    }

    #[getter]
    fn tribe(&self) -> &str {
        self.inner.tribe.as_str()
    }

    #[getter]
    fn keywords(&self) -> Vec<&'static str> {
        self.inner
            .keywords()
            .into_iter()
            .map(|k| match k {
                Keyword::Taunt => "taunt",
                Keyword::DivineShield => "divine_shield",
                Keyword::Windfury => "windfury",
                Keyword::Reborn => "reborn",
                Keyword::Venomous => "venomous",
                Keyword::Stealth => "stealth",
                Keyword::Magnetic => "magnetic",
            })
            .collect()
    }

    fn instantiate(&self) -> PyUnit {
        PyUnit {
            inner: self.inner.instantiate(),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "CardTemplate(id={}, name={:?}, stats={}/{}, tier={}, tribe={:?})",
            self.inner.card_id,
            self.inner.name,
            self.inner.attack,
            self.inner.health,
            self.inner.tavern_tier,
            self.inner.tribe.as_str()
        )
    }
}

/// Python wrapper around [`crate::combat::BattleResult`].
#[pyclass(name = "BattleResult")]
#[derive(Clone, Debug)]
pub struct PyBattleResult {
    #[pyo3(get)]
    pub outcome: String,
    #[pyo3(get)]
    pub survivors_a: Vec<PyUnit>,
    #[pyo3(get)]
    pub survivors_b: Vec<PyUnit>,
    #[pyo3(get)]
    pub hero_damage: u32,
}

/// Python wrapper around [`crate::sim::BattleDistribution`].
#[pyclass(name = "BattleDistribution")]
#[derive(Clone, Debug)]
pub struct PyDistribution {
    #[pyo3(get)]
    pub battles: u32,
    #[pyo3(get)]
    pub a_wins: u32,
    #[pyo3(get)]
    pub b_wins: u32,
    #[pyo3(get)]
    pub draws: u32,
    #[pyo3(get)]
    pub a_win_rate: f64,
    #[pyo3(get)]
    pub b_win_rate: f64,
    #[pyo3(get)]
    pub draw_rate: f64,
    #[pyo3(get)]
    pub damage_min: i32,
    #[pyo3(get)]
    pub damage_max: i32,
    #[pyo3(get)]
    pub damage_mean: f64,
}

/// Python wrapper around [`TavernAction`].
#[pyclass(name = "TavernAction")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PyTavernAction {
    pub inner: TavernAction,
}

#[pymethods]
impl PyTavernAction {
    #[staticmethod]
    fn buy(shop_index: usize) -> Self {
        Self {
            inner: TavernAction::Buy { shop_index },
        }
    }

    #[staticmethod]
    fn play(hand_index: usize, board_pos: usize) -> Self {
        Self {
            inner: TavernAction::Play {
                hand_index,
                board_pos,
            },
        }
    }

    #[staticmethod]
    fn sell(board_pos: usize) -> Self {
        Self {
            inner: TavernAction::Sell { board_pos },
        }
    }

    #[staticmethod]
    fn reposition(from_pos: usize, to_pos: usize) -> Self {
        Self {
            inner: TavernAction::Reposition { from_pos, to_pos },
        }
    }

    #[staticmethod]
    #[pyo3(signature = (board_pos, target_pos=None))]
    fn activate(board_pos: usize, target_pos: Option<usize>) -> Self {
        Self {
            inner: TavernAction::Activate {
                board_pos,
                target_pos,
            },
        }
    }

    #[staticmethod]
    fn refresh() -> Self {
        Self {
            inner: TavernAction::Refresh,
        }
    }

    #[staticmethod]
    fn upgrade_tavern() -> Self {
        Self {
            inner: TavernAction::UpgradeTavern,
        }
    }

    #[staticmethod]
    fn toggle_freeze() -> Self {
        Self {
            inner: TavernAction::ToggleFreeze,
        }
    }

    #[staticmethod]
    fn choose_discover(option_index: usize) -> Self {
        Self {
            inner: TavernAction::ChooseDiscover { option_index },
        }
    }

    #[staticmethod]
    fn end_turn() -> Self {
        Self {
            inner: TavernAction::EndTurn,
        }
    }

    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            TavernAction::Buy { .. } => "buy",
            TavernAction::Play { .. } => "play",
            TavernAction::Sell { .. } => "sell",
            TavernAction::Reposition { .. } => "reposition",
            TavernAction::Activate { .. } => "activate",
            TavernAction::Refresh => "refresh",
            TavernAction::UpgradeTavern => "upgrade_tavern",
            TavernAction::ToggleFreeze => "toggle_freeze",
            TavernAction::ChooseDiscover { .. } => "choose_discover",
            TavernAction::EndTurn => "end_turn",
        }
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }
}

/// Snapshot of [`TavernState`] exposed to Python.
#[pyclass(name = "TavernObservation")]
#[derive(Clone, Debug)]
pub struct PyTavernObservation {
    #[pyo3(get)]
    pub turn: u32,
    #[pyo3(get)]
    pub health: i32,
    #[pyo3(get)]
    pub tavern_tier: u32,
    #[pyo3(get)]
    pub max_gold: u32,
    #[pyo3(get)]
    pub gold: u32,
    #[pyo3(get)]
    pub bonus_gold_next_turn: u32,
    #[pyo3(get)]
    pub upgrade_cost: u32,
    #[pyo3(get)]
    pub is_frozen: bool,
    #[pyo3(get)]
    pub board: Vec<PyUnit>,
    #[pyo3(get)]
    pub hand: Vec<PyUnit>,
    #[pyo3(get)]
    pub shop: Vec<PyUnit>,
    #[pyo3(get)]
    pub discover_pending: Option<Vec<PyUnit>>,
    #[pyo3(get)]
    pub deity: String,
    #[pyo3(get)]
    pub deity_attack: i32,
    #[pyo3(get)]
    pub deity_health: i32,
}

impl From<&TavernState> for PyTavernObservation {
    fn from(s: &TavernState) -> Self {
        Self {
            turn: s.turn,
            health: s.health,
            tavern_tier: s.tavern_tier,
            max_gold: s.max_gold,
            gold: s.gold,
            bonus_gold_next_turn: s.bonus_gold_next_turn,
            upgrade_cost: s.upgrade_cost,
            is_frozen: s.is_frozen,
            board: s
                .board
                .iter()
                .cloned()
                .map(|inner| PyUnit { inner })
                .collect(),
            hand: s
                .hand
                .iter()
                .cloned()
                .map(|inner| PyUnit { inner })
                .collect(),
            shop: s
                .shop
                .iter()
                .cloned()
                .map(|inner| PyUnit { inner })
                .collect(),
            discover_pending: s.discover_pending.as_ref().map(|opts| {
                opts.iter()
                    .cloned()
                    .map(|inner| PyUnit { inner })
                    .collect()
            }),
            deity: deity_kind_str(s.auras.deity.kind).to_string(),
            deity_attack: s.auras.deity.attack,
            deity_health: s.auras.deity.health,
        }
    }
}

/// Stateful handle wrapping [`TavernState`], [`CardPool`], and [`Rng`].
#[pyclass(name = "TavernGame")]
#[derive(Clone, Debug)]
pub struct PyTavernGame {
    state: TavernState,
    pool: CardPool,
    rng: Rng,
}

#[pymethods]
impl PyTavernGame {
    #[new]
    #[pyo3(signature = (seed=0, catalog="tier1", deity=None))]
    fn new(seed: u64, catalog: &str, deity: Option<&str>) -> PyResult<Self> {
        let templates = cards::catalog_for(catalog).map_err(PyValueError::new_err)?;
        let pool = CardPool::new(templates);
        let rng = Rng::new(seed);
        let mut state = TavernState::new();
        state.auras.deity.kind = parse_deity_kind(deity)?;
        Ok(Self { state, pool, rng })
    }

    fn state(&self) -> PyTavernObservation {
        PyTavernObservation::from(&self.state)
    }

    fn observe(&self) -> PyTavernObservation {
        PyTavernObservation::from(&self.state)
    }

    fn start_turn(&mut self) {
        self.state.start_turn(&mut self.pool, &mut self.rng);
    }

    fn end_turn(&mut self) -> PyResult<()> {
        self.state
            .step(TavernAction::EndTurn, &mut self.pool, &mut self.rng)
            .map(|_| ())
            .map_err(PyValueError::new_err)
    }

    fn is_legal(&self, action: &PyTavernAction) -> bool {
        self.state.is_legal(&action.inner)
    }

    fn legal_actions(&self) -> Vec<PyTavernAction> {
        self.state
            .valid_actions()
            .into_iter()
            .map(|inner| PyTavernAction { inner })
            .collect()
    }

    fn step(&mut self, action: &PyTavernAction) -> PyResult<bool> {
        self.state
            .step(action.inner, &mut self.pool, &mut self.rng)
            .map_err(PyValueError::new_err)
    }

    #[pyo3(signature = (deity=None))]
    fn set_deity(&mut self, deity: Option<&str>) -> PyResult<()> {
        self.state.auras.deity.kind = parse_deity_kind(deity)?;
        Ok(())
    }
}

#[pyfunction]
#[pyo3(signature = (board_a, board_b, seed, hero_tier_a=1, hero_tier_b=1, deity_a=None, deity_b=None, deity_stats_a=(1, 1), deity_stats_b=(1, 1)))]
#[allow(clippy::too_many_arguments)]
fn simulate(
    board_a: Vec<PyUnit>,
    board_b: Vec<PyUnit>,
    seed: u64,
    hero_tier_a: u32,
    hero_tier_b: u32,
    deity_a: Option<&str>,
    deity_b: Option<&str>,
    deity_stats_a: (i32, i32),
    deity_stats_b: (i32, i32),
) -> PyResult<PyBattleResult> {
    let units_a: Vec<Unit> = board_a.into_iter().map(|u| u.inner).collect();
    let units_b: Vec<Unit> = board_b.into_iter().map(|u| u.inner).collect();
    let mut state = GameState {
        hero_tier_a,
        hero_tier_b,
        ..GameState::default()
    };
    state.auras_a.deity.kind = parse_deity_kind(deity_a)?;
    state.auras_a.deity.attack = deity_stats_a.0;
    state.auras_a.deity.health = deity_stats_a.1;
    state.auras_b.deity.kind = parse_deity_kind(deity_b)?;
    state.auras_b.deity.attack = deity_stats_b.0;
    state.auras_b.deity.health = deity_stats_b.1;

    let res = rust_simulate(&units_a, &units_b, &state, seed);
    let outcome = match res.outcome {
        BattleOutcome::AWin => "a_win",
        BattleOutcome::BWin => "b_win",
        BattleOutcome::Draw => "draw",
    }
    .to_string();
    Ok(PyBattleResult {
        outcome,
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
    })
}

#[pyfunction]
#[pyo3(signature = (board_a, board_b, base_seed, n, hero_tier_a=1, hero_tier_b=1, deity_a=None, deity_b=None, deity_stats_a=(1, 1), deity_stats_b=(1, 1)))]
#[allow(clippy::too_many_arguments)]
fn simulate_batch(
    board_a: Vec<PyUnit>,
    board_b: Vec<PyUnit>,
    base_seed: u64,
    n: u32,
    hero_tier_a: u32,
    hero_tier_b: u32,
    deity_a: Option<&str>,
    deity_b: Option<&str>,
    deity_stats_a: (i32, i32),
    deity_stats_b: (i32, i32),
) -> PyResult<PyDistribution> {
    if n == 0 {
        return Err(PyValueError::new_err("simulate_batch requires n > 0"));
    }
    let units_a: Vec<Unit> = board_a.into_iter().map(|u| u.inner).collect();
    let units_b: Vec<Unit> = board_b.into_iter().map(|u| u.inner).collect();
    let mut state = GameState {
        hero_tier_a,
        hero_tier_b,
        ..GameState::default()
    };
    state.auras_a.deity.kind = parse_deity_kind(deity_a)?;
    state.auras_a.deity.attack = deity_stats_a.0;
    state.auras_a.deity.health = deity_stats_a.1;
    state.auras_b.deity.kind = parse_deity_kind(deity_b)?;
    state.auras_b.deity.attack = deity_stats_b.0;
    state.auras_b.deity.health = deity_stats_b.1;

    let dist = rust_simulate_batch(&units_a, &units_b, &state, base_seed, n);
    Ok(PyDistribution {
        battles: dist.battles,
        a_wins: dist.a_wins,
        b_wins: dist.b_wins,
        draws: dist.draws,
        a_win_rate: dist.a_win_rate,
        b_win_rate: dist.b_win_rate,
        draw_rate: dist.draw_rate,
        damage_min: dist.damage.min,
        damage_max: dist.damage.max,
        damage_mean: dist.damage.mean,
    })
}

#[pyfunction]
#[pyo3(signature = (name="tier1"))]
fn catalog(name: &str) -> PyResult<Vec<PyCardTemplate>> {
    let templates = cards::catalog_for(name).map_err(PyValueError::new_err)?;
    Ok(templates
        .into_iter()
        .map(|inner| PyCardTemplate { inner })
        .collect())
}

#[pyfunction]
fn parse_unit(spec: &str) -> PyResult<PyUnit> {
    let inner = rust_parse_unit(spec, &Defaults::default()).map_err(PyValueError::new_err)?;
    Ok(PyUnit { inner })
}

#[pymodule]
pub fn seaglass(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyUnit>()?;
    m.add_class::<PyCardTemplate>()?;
    m.add_class::<PyBattleResult>()?;
    m.add_class::<PyDistribution>()?;
    m.add_class::<PyTavernAction>()?;
    m.add_class::<PyTavernObservation>()?;
    m.add_class::<PyTavernGame>()?;
    m.add_function(wrap_pyfunction!(simulate, m)?)?;
    m.add_function(wrap_pyfunction!(simulate_batch, m)?)?;
    m.add_function(wrap_pyfunction!(catalog, m)?)?;
    m.add_function(wrap_pyfunction!(parse_unit, m)?)?;
    Ok(())
}
