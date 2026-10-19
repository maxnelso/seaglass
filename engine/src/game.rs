//! Recorded multi-player matches (`docs/replays.md`).
//!
//! A [`Match`] owns every player's [`TavernState`] together with the card pools and RNGs they draw
//! from and the [`Lobby`] they see each other through, and records each engine call into a replay
//! ([`crate::replay`]) as the game loop makes it.

use std::collections::BTreeMap;
use std::path::Path;

use crate::cards;
use crate::combat::BattleResult;
use crate::lobby::Lobby;
use crate::model::{DeityKind, HeroId};
use crate::replay::{self, pb};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernAction, TavernState};

use pb::record::Kind;

/// How the players' card pools and RNGs are shared ([`pb::PoolMode`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PoolMode {
    /// Every player has their own card pool and RNG, seeded by [`PlayerSetup::seed`].
    #[default]
    PerPlayer,
    /// All players share one card pool and one RNG, seeded by [`MatchSetup::shared_seed`].
    Shared,
}

/// One player's starting setup ([`pb::PlayerSetup`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerSetup {
    pub name: String,
    /// The seed of this player's RNG ([`PoolMode::PerPlayer`]).
    pub seed: u64,
    pub deity: DeityKind,
    pub armor: i32,
    /// Whether Bob's shop also offers a Tavern spell ([`TavernState::with_shop_spells`]).
    pub shop_spells: bool,
    /// The player's hero ([`TavernState::set_hero`]): its Health, Armor and Hero Power. `None`:
    /// no hero, and [`Self::armor`] Armor.
    pub hero: Option<HeroId>,
    /// The trinket offers of turns 6 and 9 ([`TavernState::trinkets_enabled`]).
    pub trinkets: bool,
    /// The Dark Discovery button ([`TavernState::dark_gifts_enabled`]).
    pub dark_gifts: bool,
}

impl Default for PlayerSetup {
    /// The defaults of the Python `TavernGame`: no Deity, no hero, 5 Armor, shop spells, trinkets
    /// and Dark Gifts.
    fn default() -> Self {
        Self {
            name: String::new(),
            seed: 0,
            deity: DeityKind::None,
            armor: 5,
            shop_spells: true,
            hero: None,
            trinkets: true,
            dark_gifts: true,
        }
    }
}

/// A match's setup ([`pb::Header`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchSetup {
    /// The card catalog ([`cards::catalog_for`]).
    pub catalog: String,
    pub pool: PoolMode,
    /// The seed of the shared RNG ([`PoolMode::Shared`]).
    pub shared_seed: u64,
    pub players: Vec<PlayerSetup>,
    /// Free-form labels stored in the replay (source, run, iteration, opponent kind, ...).
    pub meta: BTreeMap<String, String>,
}

impl Default for MatchSetup {
    fn default() -> Self {
        Self {
            catalog: "full".to_string(),
            pool: PoolMode::PerPlayer,
            shared_seed: 0,
            players: Vec::new(),
            meta: BTreeMap::new(),
        }
    }
}

impl MatchSetup {
    /// The replay header of a match with this setup, stamped with this engine build.
    pub fn to_header(&self) -> pb::Header {
        let pool = match self.pool {
            PoolMode::PerPlayer => pb::PoolMode::PerPlayer,
            PoolMode::Shared => pb::PoolMode::Shared,
        };
        pb::Header {
            format: replay::FORMAT_VERSION,
            engine: replay::engine_version().to_string(),
            catalog: self.catalog.clone(),
            pool: pool as i32,
            shared_seed: self.shared_seed,
            players: self
                .players
                .iter()
                .map(|p| pb::PlayerSetup {
                    name: p.name.clone(),
                    seed: p.seed,
                    deity: replay::deity_to_pb(p.deity) as i32,
                    armor: p.armor,
                    shop_spells: p.shop_spells,
                    hero: p.hero,
                    trinkets: p.trinkets,
                    dark_gifts: p.dark_gifts,
                })
                .collect(),
            meta: self.meta.clone(),
        }
    }

    /// The setup a replay header records.
    pub fn from_header(header: &pb::Header) -> Result<Self, String> {
        if header.format != replay::FORMAT_VERSION {
            return Err(format!(
                "replay format {} is not supported (this engine reads format {}): the replay was \
                 recorded by seaglass {}; re-record it with this build (e.g. `sim_match --record`), \
                 or view it with the commit that recorded it",
                header.format,
                replay::FORMAT_VERSION,
                if header.engine.is_empty() {
                    "(unknown)"
                } else {
                    &header.engine
                }
            ));
        }
        let pool = match pb::PoolMode::try_from(header.pool) {
            Ok(pb::PoolMode::PerPlayer) => PoolMode::PerPlayer,
            Ok(pb::PoolMode::Shared) => PoolMode::Shared,
            _ => return Err(format!("invalid pool mode {}", header.pool)),
        };
        let players = header
            .players
            .iter()
            .map(|p| {
                let deity = pb::Deity::try_from(p.deity)
                    .map_err(|_| format!("invalid deity {} for player {:?}", p.deity, p.name))?;
                Ok(PlayerSetup {
                    name: p.name.clone(),
                    seed: p.seed,
                    deity: replay::deity_from_pb(deity),
                    armor: p.armor,
                    shop_spells: p.shop_spells,
                    hero: p.hero,
                    trinkets: p.trinkets,
                    dark_gifts: p.dark_gifts,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Self {
            catalog: header.catalog.clone(),
            pool,
            shared_seed: header.shared_seed,
            players,
            meta: header.meta.clone(),
        })
    }
}

/// A match between players that records every engine call into a replay (`docs/replays.md`).
///
/// Drive it like the bare engine, with [`Self::start_turn`], [`Self::step`] and [`Self::combat`]
/// in whatever order the game loop wants, and the lobby with [`Self::set_pairs`],
/// [`Self::deliver_gifts`] and [`Self::record_deaths`], then save [`Self::replay`]. Each call is recorded after
/// its legality check and before it runs, so the replay of a game that panicked ends with the call
/// that panicked.
///
/// Players are indexed `0..num_players()`; passing any other index panics.
#[derive(Clone, Debug)]
pub struct Match {
    header: pb::Header,
    pool_mode: PoolMode,
    players: Vec<TavernState>,
    pools: Vec<CardPool>,
    rngs: Vec<Rng>,
    lobby: Lobby,
    records: Vec<pb::Record>,
}

impl Match {
    /// Start a match: every player at Turn 0 (call [`Self::start_turn`] to begin Turn 1).
    pub fn new(setup: &MatchSetup) -> Result<Self, String> {
        if setup.players.is_empty() {
            return Err("a match needs at least one player".to_string());
        }
        let templates = cards::catalog_for(&setup.catalog)?;
        let players = setup
            .players
            .iter()
            .map(|p| {
                let mut state = TavernState::new().with_shop_spells(p.shop_spells);
                state.trinkets_enabled = p.trinkets;
                state.dark_gifts_enabled = p.dark_gifts;
                state.auras.deity.kind = p.deity;
                match p.hero {
                    Some(id) => state
                        .set_hero(id)
                        .map_err(|e| format!("player {:?}: {e}", p.name))?,
                    None => state.armor = p.armor,
                }
                Ok(state)
            })
            .collect::<Result<_, String>>()?;
        let (pools, rngs) = match setup.pool {
            PoolMode::Shared => (
                vec![CardPool::new(templates)],
                vec![Rng::new(setup.shared_seed)],
            ),
            PoolMode::PerPlayer => (
                vec![CardPool::new(templates); setup.players.len()],
                setup.players.iter().map(|p| Rng::new(p.seed)).collect(),
            ),
        };
        Ok(Self {
            header: setup.to_header(),
            pool_mode: setup.pool,
            players,
            pools,
            rngs,
            lobby: Lobby::new(setup.players.len(), false),
            records: Vec::new(),
        })
    }

    /// Start a match with the setup a replay header records.
    pub fn from_header(header: &pb::Header) -> Result<Self, String> {
        Self::new(&MatchSetup::from_header(header)?)
    }

    pub fn num_players(&self) -> usize {
        self.players.len()
    }

    pub fn pool_mode(&self) -> PoolMode {
        self.pool_mode
    }

    pub fn player(&self, p: usize) -> &TavernState {
        &self.players[p]
    }

    pub fn players(&self) -> &[TavernState] {
        &self.players
    }

    /// The card pool player `p` draws from.
    pub fn pool(&self, p: usize) -> &CardPool {
        &self.pools[self.slot(p)]
    }

    /// The RNG player `p` draws from.
    pub fn rng(&self, p: usize) -> &Rng {
        &self.rngs[self.slot(p)]
    }

    /// Every card pool: one per player ([`PoolMode::PerPlayer`]) or one shared.
    pub fn pools(&self) -> &[CardPool] {
        &self.pools
    }

    /// Every RNG: one per player ([`PoolMode::PerPlayer`]) or one shared.
    pub fn rngs(&self) -> &[Rng] {
        &self.rngs
    }

    /// What the players know about each other ([`Self::set_pairs`]).
    pub fn lobby(&self) -> &Lobby {
        &self.lobby
    }

    pub fn header(&self) -> &pb::Header {
        &self.header
    }

    /// The replay's free-form labels ([`MatchSetup::meta`]).
    pub fn meta_mut(&mut self) -> &mut BTreeMap<String, String> {
        &mut self.header.meta
    }

    /// The calls recorded so far.
    pub fn records(&self) -> &[pb::Record] {
        &self.records
    }

    /// The replay of the match so far.
    pub fn replay(&self) -> pb::Replay {
        pb::Replay {
            header: Some(self.header.clone()),
            records: self.records.clone(),
        }
    }

    /// Save the replay of the match so far ([`replay::save`]).
    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        replay::save(&self.replay(), path)
    }

    /// [`TavernState::start_turn`] for player `p`.
    pub fn start_turn(&mut self, p: usize) {
        self.check_player(p);
        let turn = self.players[p].turn + 1;
        self.records.push(record(Kind::StartTurn(pb::StartTurn {
            player: p as u32,
            turn,
        })));
        let s = self.slot(p);
        self.players[p].start_turn(&mut self.pools[s], &mut self.rngs[s]);
    }

    /// [`TavernState::step`] for player `p`. Illegal actions return `Err` and are not recorded.
    pub fn step(&mut self, p: usize, action: TavernAction) -> Result<bool, String> {
        self.step_with_note(p, action, None)
    }

    /// [`Self::step`], recording what the actor knew or thought about the action ([`pb::Note`]).
    pub fn step_with_note(
        &mut self,
        p: usize,
        action: TavernAction,
        note: Option<pb::Note>,
    ) -> Result<bool, String> {
        self.check_player(p);
        let state = &self.players[p];
        if !state.is_legal(&action) {
            return Err(format!("illegal tavern action: {action:?}"));
        }
        let card_id = replay::action_card(state, &action);
        self.records.push(record(Kind::Act(pb::Act {
            player: p as u32,
            action: Some(replay::action_to_pb(&action)),
            card_id,
            note,
        })));
        let s = self.slot(p);
        let result = self.players[p].step(action, &mut self.pools[s], &mut self.rngs[s]);
        if result.is_err() {
            self.records.pop();
        }
        result
    }

    /// `EndTurn` for player `p`.
    pub fn end_turn(&mut self, p: usize) -> Result<bool, String> {
        self.step(p, TavernAction::EndTurn)
    }

    /// The coming turn's pairings: [`Lobby::set_next_pairs`], then [`Lobby::apply`] (every
    /// player's Tavern gets their view). Call it before the players start their turns.
    pub fn set_pairs(&mut self, pairs: &[(usize, usize)]) {
        for &(a, b) in pairs {
            self.check_pair(a, b);
        }
        self.records.push(record(Kind::SetPairs(pb::SetPairs {
            pairs: pairs
                .iter()
                .map(|&(a, b)| pb::Pair {
                    a: a as u32,
                    b: b as u32,
                })
                .collect(),
        })));
        self.lobby.set_next_pairs(pairs);
        let mut players: Vec<&mut TavernState> = self.players.iter_mut().collect();
        self.lobby.apply(&mut players);
    }

    /// [`Lobby::deliver_gifts`]: every player's lobby gifts reach their living opponents.
    pub fn deliver_gifts(&mut self) {
        self.records
            .push(record(Kind::DeliverGifts(pb::DeliverGifts {})));
        let mut players: Vec<&mut TavernState> = self.players.iter_mut().collect();
        self.lobby.deliver_gifts(&mut players);
    }

    /// The combats are over: [`Lobby::record_deaths`] with every player still above 0 Health
    /// alive. Returns who is alive.
    pub fn record_deaths(&mut self) -> Vec<bool> {
        let alive: Vec<bool> = self.players.iter().map(|s| s.health > 0).collect();
        self.records
            .push(record(Kind::RecordDeaths(pb::RecordDeaths {
                alive: alive.clone(),
            })));
        self.lobby.record_deaths(&alive);
        alive
    }

    /// [`TavernState::resolve_combat_pair`] between players `a` and `b`, with a seed drawn from the
    /// RNG player `a` draws from. The lobby records both players' combats and the outcome
    /// ([`Lobby::record_combat`], [`Lobby::record_outcome`]).
    pub fn combat(&mut self, a: usize, b: usize) -> BattleResult {
        self.check_pair(a, b);
        let s = self.slot(a);
        let seed = self.rngs[s].next_u64();
        self.resolve_combat(a, b, seed, false)
    }

    /// [`Self::combat`] with a seed chosen by the caller (no RNG draw).
    pub fn combat_with_seed(&mut self, a: usize, b: usize, seed: u64) -> BattleResult {
        self.check_pair(a, b);
        self.resolve_combat(a, b, seed, true)
    }

    /// Record the end of the match and return its winner: the player with the most Health, or
    /// `None` if several players tie for the most.
    pub fn finish(&mut self) -> Option<usize> {
        let best = self.players.iter().map(|s| s.health).max().unwrap_or(0);
        let mut leaders = (0..self.players.len()).filter(|&p| self.players[p].health == best);
        let winner = match (leaders.next(), leaders.next()) {
            (Some(p), None) => Some(p),
            _ => None,
        };
        self.records.push(record(Kind::End(pb::End {
            winner: winner.map(|p| p as u32),
            turn: self.players.iter().map(|s| s.turn).max().unwrap_or(0),
            health: self.players.iter().map(|s| s.health).collect(),
        })));
        winner
    }

    fn resolve_combat(
        &mut self,
        a: usize,
        b: usize,
        seed: u64,
        external_seed: bool,
    ) -> BattleResult {
        self.records.push(record(Kind::Combat(pb::Combat {
            a: a as u32,
            b: b as u32,
            seed,
            external_seed,
            ..Default::default()
        })));
        let (board_a, board_b) = (self.players[a].board.clone(), self.players[b].board.clone());
        let (player_a, player_b) = pair_mut(&mut self.players, a, b);
        let result = TavernState::resolve_combat_pair(player_a, player_b, seed);
        self.lobby.record_combat(a, Some(b), &board_a);
        self.lobby.record_combat(b, Some(a), &board_b);
        self.lobby.record_outcome(a, b, result.outcome);
        let (check_a, check_b) = (
            replay::checkpoint(&self.players[a]),
            replay::checkpoint(&self.players[b]),
        );
        if let Some(pb::Record {
            kind: Some(Kind::Combat(combat)),
        }) = self.records.last_mut()
        {
            combat.set_outcome(replay::outcome_to_pb(result.outcome));
            combat.hero_damage = result.hero_damage;
            combat.check_a = Some(check_a);
            combat.check_b = Some(check_b);
        }
        result
    }

    fn slot(&self, p: usize) -> usize {
        match self.pool_mode {
            PoolMode::PerPlayer => p,
            PoolMode::Shared => 0,
        }
    }

    fn check_player(&self, p: usize) {
        assert!(
            p < self.players.len(),
            "player {p} does not exist (the match has {} players)",
            self.players.len()
        );
    }

    fn check_pair(&self, a: usize, b: usize) {
        self.check_player(a);
        self.check_player(b);
        assert_ne!(a, b, "player {a} cannot fight themselves");
    }
}

fn record(kind: Kind) -> pb::Record {
    pb::Record { kind: Some(kind) }
}

/// Mutable references to two different elements of `items`.
fn pair_mut<T>(items: &mut [T], a: usize, b: usize) -> (&mut T, &mut T) {
    assert_ne!(a, b);
    if a < b {
        let (low, high) = items.split_at_mut(b);
        (&mut low[a], &mut high[0])
    } else {
        let (low, high) = items.split_at_mut(a);
        (&mut high[0], &mut low[b])
    }
}
