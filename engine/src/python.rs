//! Thin marshalling-only PyO3 bindings (`--features python`).
//!
//! All RL-specific decisions (action flattening, reward shaping, observation tensors,
//! and synthetic opponents) belong in `rl/` rather than here.

#![allow(clippy::useless_conversion)]

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use crate::cards::{self, CardTemplate};
use crate::combat::{DuosResult, DuosTeam, Warband};
use crate::duos_tavern::DuosTavern;
use crate::heroes;
use crate::model::{BattleOutcome, DeityKind, GameState, HeroId, Keyword, Unit};
use crate::rng::Rng;
use crate::scenario::{parse_unit as rust_parse_unit, Defaults};
use crate::sim::{
    simulate as rust_simulate, simulate_batch as rust_simulate_batch,
    simulate_duos as rust_simulate_duos, simulate_duos_batch as rust_simulate_duos_batch,
    BattleDistribution,
};
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
        let unit = rust_parse_unit(spec, &Defaults::default()).map_err(PyValueError::new_err)?;
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

    /// The card's counter: a `Triple Reward` card's locked-in Discover Tier (`docs/tavern.md` §5.2),
    /// and other cards' progress.
    #[getter]
    fn counter(&self) -> i32 {
        self.inner.counter
    }

    /// A card in the shop frozen on its own (`docs/tavern.md` §3.1).
    #[getter]
    fn frozen(&self) -> bool {
        self.inner.frozen
    }

    #[getter]
    fn activated_this_turn(&self) -> bool {
        self.inner.activated_this_turn
    }

    #[getter]
    fn spell_cost(&self) -> u32 {
        self.inner.spell_cost
    }

    #[getter]
    fn costs_health(&self) -> bool {
        self.inner.costs_health
    }

    #[getter]
    fn unplayable(&self) -> bool {
        self.inner.unplayable || self.inner.lockbox_turns_left > 0 || self.inner.locked_turns > 0
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
    fn play_on_shop(hand_index: usize, shop_index: usize) -> Self {
        Self {
            inner: TavernAction::PlayOnShop {
                hand_index,
                shop_index,
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

    /// Duos: Pass `hand[hand_index]` to the teammate (`docs/duos_tavern.md` §5). A Solo
    /// `TavernGame` never allows it.
    #[staticmethod]
    fn pass_card(hand_index: usize) -> Self {
        Self {
            inner: TavernAction::Pass { hand_index },
        }
    }

    /// Use the Hero Power, on `target` if it needs one (`docs/heroes.md` §5).
    #[staticmethod]
    #[pyo3(signature = (target=None))]
    fn hero_power(target: Option<usize>) -> Self {
        Self {
            inner: TavernAction::HeroPower { target },
        }
    }

    /// Choose the offered hero `hero_options[option_index]` (`docs/heroes.md` §6).
    #[staticmethod]
    fn choose_hero(option_index: usize) -> Self {
        Self {
            inner: TavernAction::ChooseHero { option_index },
        }
    }

    #[getter]
    fn kind(&self) -> &'static str {
        match self.inner {
            TavernAction::Buy { .. } => "buy",
            TavernAction::Play { .. } => "play",
            TavernAction::PlayOnShop { .. } => "play_on_shop",
            TavernAction::Sell { .. } => "sell",
            TavernAction::Reposition { .. } => "reposition",
            TavernAction::Activate { .. } => "activate",
            TavernAction::Refresh => "refresh",
            TavernAction::UpgradeTavern => "upgrade_tavern",
            TavernAction::ToggleFreeze => "toggle_freeze",
            TavernAction::ChooseDiscover { .. } => "choose_discover",
            TavernAction::EndTurn => "end_turn",
            TavernAction::Pass { .. } => "pass",
            TavernAction::HeroPower { .. } => "hero_power",
            TavernAction::ChooseHero { .. } => "choose_hero",
        }
    }

    #[getter]
    fn args(&self) -> (Option<usize>, Option<usize>) {
        match self.inner {
            TavernAction::Buy { shop_index } => (Some(shop_index), None),
            TavernAction::Play {
                hand_index,
                board_pos,
            } => (Some(hand_index), Some(board_pos)),
            TavernAction::PlayOnShop {
                hand_index,
                shop_index,
            } => (Some(hand_index), Some(shop_index)),
            TavernAction::Sell { board_pos } => (Some(board_pos), None),
            TavernAction::Reposition { from_pos, to_pos } => (Some(from_pos), Some(to_pos)),
            TavernAction::Activate {
                board_pos,
                target_pos,
            } => (Some(board_pos), target_pos),
            TavernAction::ChooseDiscover { option_index } => (Some(option_index), None),
            TavernAction::Pass { hand_index } => (Some(hand_index), None),
            TavernAction::HeroPower { target } => (target, None),
            TavernAction::ChooseHero { option_index } => (Some(option_index), None),
            TavernAction::Refresh
            | TavernAction::UpgradeTavern
            | TavernAction::ToggleFreeze
            | TavernAction::EndTurn => (None, None),
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
    pub armor: i32,
    #[pyo3(get)]
    pub tavern_tier: u32,
    #[pyo3(get)]
    pub max_gold: u32,
    #[pyo3(get)]
    pub gold: u32,
    #[pyo3(get)]
    pub bonus_gold_next_turn: u32,
    #[pyo3(get)]
    pub free_refreshes: u32,
    #[pyo3(get)]
    pub upgrade_cost: u32,
    #[pyo3(get)]
    pub is_frozen: bool,
    /// The next minion bought costs this much less (`docs/tavern.md` §6).
    #[pyo3(get)]
    pub next_minion_discount: i32,
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
    /// The hero's id (its dbfId), 0 without a hero (`docs/heroes.md` §8).
    #[pyo3(get)]
    pub hero_id: u32,
    /// The Hero Power card in use (its dbfId), 0 without a hero.
    #[pyo3(get)]
    pub hero_power_id: u32,
    #[pyo3(get)]
    pub hero_power_cost: u32,
    #[pyo3(get)]
    pub hero_power_uses_left: u32,
    #[pyo3(get)]
    pub hero_power_counter: i32,
    #[pyo3(get)]
    pub hero_power_stacks: i32,
    #[pyo3(get)]
    pub hero_power_armed: u32,
    /// `true` if some `HeroPower` action is legal now.
    #[pyo3(get)]
    pub hero_power_usable: bool,
    /// The heroes offered to choose from (their ids).
    #[pyo3(get)]
    pub hero_options: Vec<u32>,
}

impl From<&TavernState> for PyTavernObservation {
    fn from(s: &TavernState) -> Self {
        Self {
            turn: s.turn,
            health: s.health,
            armor: s.armor,
            tavern_tier: s.tavern_tier,
            max_gold: s.max_gold,
            gold: s.gold,
            bonus_gold_next_turn: s.bonus_gold_next_turn,
            free_refreshes: s.auras.free_refreshes,
            upgrade_cost: s.upgrade_cost,
            is_frozen: s.is_frozen,
            next_minion_discount: s.auras.next_minion_discount,
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
            discover_pending: s
                .discover_pending
                .as_ref()
                .map(|opts| opts.iter().cloned().map(|inner| PyUnit { inner }).collect()),
            deity: deity_kind_str(s.auras.deity.kind).to_string(),
            deity_attack: s.auras.deity.attack,
            deity_health: s.auras.deity.health,
            hero_id: s.hero().map_or(0, |h| h.hero_id),
            hero_power_id: s.hero().map_or(0, |h| h.power.card_id),
            hero_power_cost: s.hero().map_or(0, |h| h.power.cost),
            hero_power_uses_left: s.hero().map_or(0, |h| h.power.uses_left),
            hero_power_counter: s.hero().map_or(0, |h| h.power.counter),
            hero_power_stacks: s.hero().map_or(0, |h| h.power.stacks),
            hero_power_armed: s.hero().map_or(0, |h| h.power.armed),
            hero_power_usable: s.discover_pending.is_none()
                && s.hero_options.is_empty()
                && !heroes::valid_targets(s).is_empty(),
            hero_options: s.hero_options.clone(),
        }
    }
}

/// Each Duos player's hero, as given to `DuosTavernGame` (a name, an id or `None`).
type HeroPair<'py> = (Option<Bound<'py, PyAny>>, Option<Bound<'py, PyAny>>);

/// A hero given as its id (dbfId) or its name (`docs/heroes.md` §2.1).
fn parse_hero(hero: Option<&Bound<'_, PyAny>>) -> PyResult<Option<HeroId>> {
    let Some(hero) = hero.filter(|h| !h.is_none()) else {
        return Ok(None);
    };
    let found = if let Ok(id) = hero.extract::<u32>() {
        heroes::data(id)
    } else {
        heroes::by_name(&hero.extract::<String>()?)
    };
    found
        .map(|h| Some(h.id))
        .ok_or_else(|| PyValueError::new_err(format!("unknown hero {hero}")))
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
    /// `hero` (a name or id; `docs/heroes.md`) gives the player that hero: its Health, Armor and
    /// Hero Power. Without one (the default) the player has `armor` and no Hero Power.
    #[new]
    #[pyo3(signature = (seed=0, catalog="full", deity=None, shop_spells=true, armor=5, hero=None))]
    fn new(
        seed: u64,
        catalog: &str,
        deity: Option<&str>,
        shop_spells: bool,
        armor: i32,
        hero: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let templates = cards::catalog_for(catalog).map_err(PyValueError::new_err)?;
        let pool = CardPool::new(templates);
        let rng = Rng::new(seed);
        let mut state = TavernState::new().with_shop_spells(shop_spells);
        state.auras.deity.kind = parse_deity_kind(deity)?;
        match parse_hero(hero)? {
            Some(id) => state.set_hero(id).map_err(PyValueError::new_err)?,
            None => state.armor = armor,
        }
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

    #[pyo3(signature = (opponent_board, opponent_tier=1, seed=None))]
    fn resolve_combat_against(
        &mut self,
        opponent_board: Vec<PyUnit>,
        opponent_tier: u32,
        seed: Option<u64>,
    ) -> PyBattleResult {
        let units_b: Vec<Unit> = opponent_board.into_iter().map(|u| u.inner).collect();
        let combat_seed = seed.unwrap_or_else(|| self.rng.next_u64());
        let res = self.state.resolve_combat_against(
            &units_b,
            opponent_tier,
            &crate::model::PlayerAuras::default(),
            &[],
            combat_seed,
        );
        let outcome = match res.outcome {
            BattleOutcome::AWin => "a_win",
            BattleOutcome::BWin => "b_win",
            BattleOutcome::Draw => "draw",
        }
        .to_string();
        PyBattleResult {
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
        }
    }

    #[pyo3(signature = (other, seed=None))]
    fn resolve_combat_with(
        &mut self,
        other: &mut PyTavernGame,
        seed: Option<u64>,
    ) -> PyBattleResult {
        let combat_seed = seed.unwrap_or_else(|| self.rng.next_u64());
        let res = TavernState::resolve_combat_pair(&mut self.state, &mut other.state, combat_seed);
        let outcome = match res.outcome {
            BattleOutcome::AWin => "a_win",
            BattleOutcome::BWin => "b_win",
            BattleOutcome::Draw => "draw",
        }
        .to_string();
        PyBattleResult {
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
        }
    }

    #[pyo3(signature = (opponent_board, opponent_tier=1, n=32, base_seed=None))]
    fn equity_against(
        &self,
        opponent_board: Vec<PyUnit>,
        opponent_tier: u32,
        n: u32,
        base_seed: Option<u64>,
    ) -> PyResult<PyDistribution> {
        if n == 0 {
            return Err(PyValueError::new_err("equity_against requires n > 0"));
        }
        let units_b: Vec<Unit> = opponent_board.into_iter().map(|u| u.inner).collect();
        let state = GameState {
            hero_tier_a: self.state.tavern_tier,
            hero_tier_b: opponent_tier,
            auras_a: self.state.auras.clone(),
            auras_b: crate::model::PlayerAuras::default(),
            hand_a: self.state.hand.clone(),
            hand_b: Vec::new(),
        };
        let seed = base_seed.unwrap_or(self.state.turn as u64 * 10_007 + 1);
        let dist = rust_simulate_batch(&self.state.board, &units_b, &state, seed, n);
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

    #[pyo3(signature = (deity=None))]
    fn set_deity(&mut self, deity: Option<&str>) -> PyResult<()> {
        self.state.auras.deity.kind = parse_deity_kind(deity)?;
        Ok(())
    }
}

/// `units` as Python units.
fn py_units(units: &[Unit]) -> Vec<PyUnit> {
    units
        .iter()
        .cloned()
        .map(|inner| PyUnit { inner })
        .collect()
}

/// A battle outcome's Python name: `a_win`, `b_win` or `draw`.
fn outcome_name(outcome: BattleOutcome) -> String {
    match outcome {
        BattleOutcome::AWin => "a_win",
        BattleOutcome::BWin => "b_win",
        BattleOutcome::Draw => "draw",
    }
    .to_string()
}

/// A [`BattleDistribution`] as a Python `BattleDistribution`.
fn py_distribution(dist: BattleDistribution) -> PyDistribution {
    PyDistribution {
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
    }
}

/// A Duos team from Python (`docs/duos.md` §1): its two players' boards (player 0, player 1),
/// the player whose warband fights first, and each player's Tavern Tier.
fn duos_team(boards: Vec<Vec<PyUnit>>, first: usize, hero_tiers: (u32, u32)) -> PyResult<DuosTeam> {
    if first > 1 {
        return Err(PyValueError::new_err(format!(
            "first is the player whose warband fights first: 0 or 1, got {first}"
        )));
    }
    let count = boards.len();
    let [board_0, board_1]: [Vec<PyUnit>; 2] = boards.try_into().map_err(|_| {
        PyValueError::new_err(format!(
            "a Duos team lists its two players' boards, got {count}"
        ))
    })?;
    let warband = |board: Vec<PyUnit>, hero_tier: u32| Warband {
        board: board.into_iter().map(|u| u.inner).collect(),
        hero_tier,
        ..Warband::default()
    };
    Ok(DuosTeam {
        warbands: [
            warband(board_0, hero_tiers.0),
            warband(board_1, hero_tiers.1),
        ],
        first,
    })
}

/// Python wrapper around [`DuosResult`]: a Duos combat's outcome and damage, and each player's
/// part in it (`docs/duos.md` §6).
#[pyclass(name = "DuosBattleResult")]
#[derive(Clone, Debug)]
pub struct PyDuosBattleResult {
    /// The teams' outcome: `a_win`, `b_win` or `draw`.
    #[pyo3(get)]
    pub outcome: String,
    /// The damage the losing team takes.
    #[pyo3(get)]
    pub hero_damage: u32,
    /// Whether each of team A's players' warbands fought (player 0, player 1).
    #[pyo3(get)]
    pub fought_a: Vec<bool>,
    #[pyo3(get)]
    pub fought_b: Vec<bool>,
    /// Each of team A's players' surviving minions (player 0, player 1).
    #[pyo3(get)]
    pub survivors_a: Vec<Vec<PyUnit>>,
    #[pyo3(get)]
    pub survivors_b: Vec<Vec<PyUnit>>,
}

impl From<&DuosResult> for PyDuosBattleResult {
    fn from(res: &DuosResult) -> Self {
        Self {
            outcome: outcome_name(res.outcome),
            hero_damage: res.hero_damage,
            fought_a: res.team_a.iter().map(|w| w.fought).collect(),
            fought_b: res.team_b.iter().map(|w| w.fought).collect(),
            survivors_a: res.team_a.iter().map(|w| py_units(&w.survivors)).collect(),
            survivors_b: res.team_b.iter().map(|w| py_units(&w.survivors)).collect(),
        }
    }
}

#[pymethods]
impl PyDuosBattleResult {
    fn __repr__(&self) -> String {
        format!(
            "DuosBattleResult(outcome={:?}, hero_damage={}, fought_a={:?}, fought_b={:?})",
            self.outcome, self.hero_damage, self.fought_a, self.fought_b
        )
    }
}

/// One player's view of a Duos team (`docs/duos_tavern.md`): their own Tavern (the fields of
/// `TavernObservation`; `health` and `armor` are the team's), their queue and free Passes, what
/// they see of their teammate's Tavern, and the team's.
#[pyclass(name = "DuosObservation")]
#[derive(Clone, Debug)]
pub struct PyDuosObservation {
    /// The observing player (`0` or `1`).
    #[pyo3(get)]
    pub player: usize,
    #[pyo3(get)]
    pub turn: u32,
    #[pyo3(get)]
    pub health: i32,
    #[pyo3(get)]
    pub armor: i32,
    #[pyo3(get)]
    pub tavern_tier: u32,
    #[pyo3(get)]
    pub max_gold: u32,
    #[pyo3(get)]
    pub gold: u32,
    #[pyo3(get)]
    pub bonus_gold_next_turn: u32,
    #[pyo3(get)]
    pub free_refreshes: u32,
    #[pyo3(get)]
    pub upgrade_cost: u32,
    #[pyo3(get)]
    pub is_frozen: bool,
    /// The next minion bought costs this much less (`docs/tavern.md` §6).
    #[pyo3(get)]
    pub next_minion_discount: i32,
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
    /// This player's hero and Hero Power (`docs/heroes.md` §8), as in `TavernObservation`.
    #[pyo3(get)]
    pub hero_id: u32,
    #[pyo3(get)]
    pub hero_power_id: u32,
    #[pyo3(get)]
    pub hero_power_cost: u32,
    #[pyo3(get)]
    pub hero_power_uses_left: u32,
    #[pyo3(get)]
    pub hero_power_counter: i32,
    #[pyo3(get)]
    pub hero_power_stacks: i32,
    #[pyo3(get)]
    pub hero_power_armed: u32,
    #[pyo3(get)]
    pub hero_power_usable: bool,
    #[pyo3(get)]
    pub hero_options: Vec<u32>,
    /// The teammate's hero's id, 0 without one.
    #[pyo3(get)]
    pub teammate_hero_id: u32,
    /// `true` once this player has ended their turn.
    #[pyo3(get)]
    pub ended: bool,
    /// The cards passed to this player waiting for room in their hand, first in first out.
    #[pyo3(get)]
    pub incoming: Vec<PyUnit>,
    /// This player's free Passes left this turn (`Wanderer Cho`).
    #[pyo3(get)]
    pub free_passes: u32,
    /// `true` if this player's warband fights first in the coming combat.
    #[pyo3(get)]
    pub fights_first: bool,
    #[pyo3(get)]
    pub teammate_board: Vec<PyUnit>,
    #[pyo3(get)]
    pub teammate_hand: Vec<PyUnit>,
    #[pyo3(get)]
    pub teammate_shop: Vec<PyUnit>,
    #[pyo3(get)]
    pub teammate_tavern_tier: u32,
    #[pyo3(get)]
    pub teammate_gold: u32,
    #[pyo3(get)]
    pub teammate_incoming: Vec<PyUnit>,
    #[pyo3(get)]
    pub teammate_ended: bool,
    #[pyo3(get)]
    pub teammate_free_passes: u32,
    /// The team's shared Health and Armor (the same as `health` and `armor`).
    #[pyo3(get)]
    pub team_health: i32,
    #[pyo3(get)]
    pub team_armor: i32,
    /// The team's Passes this turn and this game.
    #[pyo3(get)]
    pub passes_this_turn: u32,
    #[pyo3(get)]
    pub passes_this_game: u32,
    /// The player whose warband fights first in the coming combat.
    #[pyo3(get)]
    pub first_fighter: usize,
}

impl PyDuosObservation {
    fn new(team: &DuosTavern, player: usize) -> Self {
        let mate = DuosTavern::teammate(player);
        let own = PyTavernObservation::from(&team.players[player]);
        let other = &team.players[mate];
        Self {
            player,
            turn: own.turn,
            health: own.health,
            armor: own.armor,
            tavern_tier: own.tavern_tier,
            max_gold: own.max_gold,
            gold: own.gold,
            bonus_gold_next_turn: own.bonus_gold_next_turn,
            free_refreshes: own.free_refreshes,
            upgrade_cost: own.upgrade_cost,
            is_frozen: own.is_frozen,
            next_minion_discount: own.next_minion_discount,
            board: own.board,
            hand: own.hand,
            shop: own.shop,
            discover_pending: own.discover_pending,
            deity: own.deity,
            deity_attack: own.deity_attack,
            deity_health: own.deity_health,
            hero_id: own.hero_id,
            hero_power_id: own.hero_power_id,
            hero_power_cost: own.hero_power_cost,
            hero_power_uses_left: own.hero_power_uses_left,
            hero_power_counter: own.hero_power_counter,
            hero_power_stacks: own.hero_power_stacks,
            hero_power_armed: own.hero_power_armed,
            hero_power_usable: own.hero_power_usable && !team.ended[player],
            hero_options: own.hero_options,
            teammate_hero_id: other.hero().map_or(0, |h| h.hero_id),
            ended: team.ended[player],
            incoming: py_units(&team.incoming[player]),
            free_passes: team.free_passes(player),
            fights_first: team.first_fighter == player,
            teammate_board: py_units(&other.board),
            teammate_hand: py_units(&other.hand),
            teammate_shop: py_units(&other.shop),
            teammate_tavern_tier: other.tavern_tier,
            teammate_gold: other.gold,
            teammate_incoming: py_units(&team.incoming[mate]),
            teammate_ended: team.ended[mate],
            teammate_free_passes: team.free_passes(mate),
            team_health: team.health,
            team_armor: team.armor,
            passes_this_turn: team.passes_this_turn,
            passes_this_game: team.passes_this_game,
            first_fighter: team.first_fighter,
        }
    }
}

/// Stateful handle wrapping a Duos team's [`DuosTavern`], its [`CardPool`] and [`Rng`]
/// (`docs/duos_tavern.md`). Every player-specific call takes the player (`0` or `1`).
#[pyclass(name = "DuosTavernGame")]
#[derive(Clone, Debug)]
pub struct PyDuosTavernGame {
    team: DuosTavern,
    pool: CardPool,
    rng: Rng,
}

impl PyDuosTavernGame {
    fn check_player(player: usize) -> PyResult<()> {
        if player > 1 {
            return Err(PyValueError::new_err(format!(
                "a Duos team's players are 0 and 1, got {player}"
            )));
        }
        Ok(())
    }
}

#[pymethods]
impl PyDuosTavernGame {
    /// `armor` is each hero's starting Armor: the team starts with their sum (§3). `heroes` (a
    /// name or id for each player, or `None`; `docs/heroes.md`) gives the players heroes instead:
    /// the team gets their Duos Armor (and Patchwerk's extra Health) and each its Hero Power.
    #[new]
    #[pyo3(signature = (seed=0, catalog="duos", armor=(5, 5), first_fighter_switch_chance=0.75, shop_spells=true, deity=None, heroes=None))]
    fn new(
        seed: u64,
        catalog: &str,
        armor: (i32, i32),
        first_fighter_switch_chance: f64,
        shop_spells: bool,
        deity: Option<&str>,
        heroes: Option<HeroPair<'_>>,
    ) -> PyResult<Self> {
        if !(0.0..=1.0).contains(&first_fighter_switch_chance) {
            return Err(PyValueError::new_err(format!(
                "first_fighter_switch_chance is a probability, got {first_fighter_switch_chance}"
            )));
        }
        let templates = cards::catalog_for(catalog).map_err(PyValueError::new_err)?;
        let kind = parse_deity_kind(deity)?;
        let mut team = DuosTavern::new()
            .with_shop_spells(shop_spells)
            .with_first_fighter_switch_chance(first_fighter_switch_chance);
        for state in &mut team.players {
            state.auras.deity.kind = kind;
        }
        match heroes {
            Some((h0, h1)) => {
                let ids = [parse_hero(h0.as_ref())?, parse_hero(h1.as_ref())?];
                team = team.with_heroes(ids).map_err(PyValueError::new_err)?;
            }
            None => team = team.with_armor([armor.0, armor.1]),
        }
        Ok(Self {
            team,
            pool: CardPool::new(templates),
            rng: Rng::new(seed),
        })
    }

    /// `player`'s view of the team.
    fn observe(&self, player: usize) -> PyResult<PyDuosObservation> {
        Self::check_player(player)?;
        Ok(PyDuosObservation::new(&self.team, player))
    }

    /// Start the team's turn: both players' start of turn, then the first fighter.
    fn start_turn(&mut self) {
        self.team.start_turn(&mut self.pool, &mut self.rng);
    }

    fn is_legal(&self, player: usize, action: &PyTavernAction) -> bool {
        self.team.is_legal(player, &action.inner)
    }

    fn legal_actions(&self, player: usize) -> PyResult<Vec<PyTavernAction>> {
        Self::check_player(player)?;
        Ok(self
            .team
            .valid_actions(player)
            .into_iter()
            .map(|inner| PyTavernAction { inner })
            .collect())
    }

    /// `player` takes `action`; returns `True` if it ended their turn.
    fn step(&mut self, player: usize, action: &PyTavernAction) -> PyResult<bool> {
        self.team
            .step(player, action.inner, &mut self.pool, &mut self.rng)
            .map_err(PyValueError::new_err)
    }

    fn end_turn(&mut self, player: usize) -> PyResult<()> {
        self.step(
            player,
            &PyTavernAction {
                inner: TavernAction::EndTurn,
            },
        )
        .map(|_| ())
    }

    /// `True` once both players have ended their turn.
    fn turn_over(&self) -> bool {
        self.team.turn_over()
    }

    /// `True` if the team is out (0 Health or less).
    fn is_out(&self) -> bool {
        self.team.is_out()
    }

    #[getter]
    fn health(&self) -> i32 {
        self.team.health
    }

    #[getter]
    fn armor(&self) -> i32 {
        self.team.armor
    }

    #[getter]
    fn first_fighter(&self) -> usize {
        self.team.first_fighter
    }

    /// This team (side A) fights `other`'s (side B); both teams take their results. The seed
    /// defaults to a draw from this game's RNG.
    #[pyo3(signature = (other, seed=None))]
    fn resolve_combat_with(
        &mut self,
        other: &mut PyDuosTavernGame,
        seed: Option<u64>,
    ) -> PyDuosBattleResult {
        let combat_seed = seed.unwrap_or_else(|| self.rng.next_u64());
        let res = DuosTavern::resolve_combat_pair(&mut self.team, &mut other.team, combat_seed);
        PyDuosBattleResult::from(&res)
    }

    /// This team (side A) fights a fixed opponent team: its two players' boards, the one that
    /// fights first, and their Tavern Tiers. Only this team takes its results.
    #[pyo3(signature = (opponent, first=0, hero_tiers=(1, 1), seed=None))]
    fn resolve_combat_against(
        &mut self,
        opponent: Vec<Vec<PyUnit>>,
        first: usize,
        hero_tiers: (u32, u32),
        seed: Option<u64>,
    ) -> PyResult<PyDuosBattleResult> {
        let opponent = duos_team(opponent, first, hero_tiers)?;
        let combat_seed = seed.unwrap_or_else(|| self.rng.next_u64());
        let res = self.team.resolve_combat_against(&opponent, combat_seed);
        Ok(PyDuosBattleResult::from(&res))
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
#[pyo3(signature = (name="full"))]
fn catalog(name: &str) -> PyResult<Vec<PyCardTemplate>> {
    let templates = cards::catalog_for(name).map_err(PyValueError::new_err)?;
    Ok(templates
        .into_iter()
        .map(|inner| PyCardTemplate { inner })
        .collect())
}

/// One Duos combat (`docs/duos.md`): each team lists its two players' boards (player 0, player
/// 1), with the player whose warband fights first and each player's Tavern Tier.
#[pyfunction]
#[pyo3(signature = (team_a, team_b, seed, first_a=0, first_b=0, hero_tiers_a=(1, 1), hero_tiers_b=(1, 1)))]
fn simulate_duos(
    team_a: Vec<Vec<PyUnit>>,
    team_b: Vec<Vec<PyUnit>>,
    seed: u64,
    first_a: usize,
    first_b: usize,
    hero_tiers_a: (u32, u32),
    hero_tiers_b: (u32, u32),
) -> PyResult<PyDuosBattleResult> {
    let team_a = duos_team(team_a, first_a, hero_tiers_a)?;
    let team_b = duos_team(team_b, first_b, hero_tiers_b)?;
    Ok(PyDuosBattleResult::from(&rust_simulate_duos(
        &team_a, &team_b, seed,
    )))
}

/// `n` Duos combats from seeds derived from `base_seed` (`docs/duos.md` §5), as
/// `simulate_batch`: team A's equity.
#[pyfunction]
#[pyo3(signature = (team_a, team_b, base_seed, n, first_a=0, first_b=0, hero_tiers_a=(1, 1), hero_tiers_b=(1, 1)))]
#[allow(clippy::too_many_arguments)]
fn simulate_duos_batch(
    team_a: Vec<Vec<PyUnit>>,
    team_b: Vec<Vec<PyUnit>>,
    base_seed: u64,
    n: u32,
    first_a: usize,
    first_b: usize,
    hero_tiers_a: (u32, u32),
    hero_tiers_b: (u32, u32),
) -> PyResult<PyDistribution> {
    if n == 0 {
        return Err(PyValueError::new_err("simulate_duos_batch requires n > 0"));
    }
    let team_a = duos_team(team_a, first_a, hero_tiers_a)?;
    let team_b = duos_team(team_b, first_b, hero_tiers_b)?;
    Ok(py_distribution(rust_simulate_duos_batch(
        &team_a, &team_b, base_seed, n,
    )))
}

#[pyfunction]
fn all_card_ids() -> Vec<u32> {
    let mut ids: Vec<u32> = crate::scenario::card_index()
        .cards()
        .iter()
        .map(|c| c.card_id)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// The hero pool (`docs/heroes.md` §2.1): `(id, name, implemented)` for every hero, by name.
#[pyfunction]
fn heroes_table() -> Vec<(u32, &'static str, bool)> {
    heroes::table()
        .iter()
        .map(|h| (h.id, h.name, heroes::is_implemented(h.id)))
        .collect()
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
    m.add_class::<PyDuosBattleResult>()?;
    m.add_class::<PyDuosObservation>()?;
    m.add_class::<PyDuosTavernGame>()?;
    m.add_function(wrap_pyfunction!(simulate, m)?)?;
    m.add_function(wrap_pyfunction!(simulate_batch, m)?)?;
    m.add_function(wrap_pyfunction!(simulate_duos, m)?)?;
    m.add_function(wrap_pyfunction!(simulate_duos_batch, m)?)?;
    m.add_function(wrap_pyfunction!(catalog, m)?)?;
    m.add_function(wrap_pyfunction!(all_card_ids, m)?)?;
    m.add_function(wrap_pyfunction!(heroes_table, m)?)?;
    m.add_function(wrap_pyfunction!(parse_unit, m)?)?;
    Ok(())
}
