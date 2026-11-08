//! Recorded multi-player matches (`docs/replays.md`).
//!
//! A [`Match`] owns every player's [`TavernState`] (in Duos, every team's [`DuosTavern`]) together
//! with the card pools and RNGs they draw from and the [`Lobby`] they see each other through, and
//! records each engine call into a replay ([`crate::replay`]) as the game loop makes it.
//!
//! The *units* of a match are what fights and gets placed: its players in Solo, its teams in Duos
//! (players `2t` and `2t + 1` are team `t`).

use std::collections::BTreeMap;
use std::path::Path;

use crate::cards;
use crate::combat::{BattleResult, DuosResult, DuosTeam, Warband};
use crate::duos_tavern::DuosTavern;
use crate::lobby::Lobby;
use crate::model::{BattleOutcome, DeityKind, HeroId, PlayerAuras, Tribe, LOBBY_TRIBES};
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
    /// The heroes the player is offered at the start ([`TavernState::offer_heroes`],
    /// `docs/heroes.md` §6), if they have no [`Self::hero`]. They start with no Armor: the
    /// chosen hero gives it.
    pub hero_options: Vec<HeroId>,
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
            hero_options: Vec::new(),
        }
    }
}

/// A match's setup ([`pb::Header`]).
#[derive(Clone, Debug, PartialEq)]
pub struct MatchSetup {
    /// The card catalog ([`cards::catalog_for`]).
    pub catalog: String,
    pub pool: PoolMode,
    /// The seed of the shared RNG ([`PoolMode::Shared`]).
    pub shared_seed: u64,
    pub players: Vec<PlayerSetup>,
    /// Free-form labels stored in the replay (source, run, iteration, opponent kind, ...).
    pub meta: BTreeMap<String, String>,
    /// A Duos match (`docs/duos_tavern.md`): players `2t` and `2t + 1` are team `t`. In
    /// [`PoolMode::PerPlayer`] each team has a pool and an RNG, seeded by its first player's seed.
    pub duos: bool,
    /// Duos: each team's chance of switching its first fighter each turn (`duos_tavern.md` §4.3).
    pub first_fighter_switch_chance: f64,
    /// The minion types in the lobby (`docs/lobby_game.md` §1); empty: every type. The card pool
    /// keeps only the minions with one of them (a two-type minion through either), with no type,
    /// or with every type.
    pub lobby_tribes: Vec<Tribe>,
}

impl Default for MatchSetup {
    fn default() -> Self {
        Self {
            catalog: "full".to_string(),
            pool: PoolMode::PerPlayer,
            shared_seed: 0,
            players: Vec::new(),
            meta: BTreeMap::new(),
            duos: false,
            first_fighter_switch_chance: crate::duos_tavern::DEFAULT_FIRST_FIGHTER_SWITCH_CHANCE,
            lobby_tribes: Vec::new(),
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
                    hero_options: p.hero_options.clone(),
                })
                .collect(),
            meta: self.meta.clone(),
            duos: self.duos,
            first_fighter_switch_chance: if self.duos {
                self.first_fighter_switch_chance
            } else {
                0.0
            },
            lobby_tribes: self
                .lobby_tribes
                .iter()
                .filter_map(|t| LOBBY_TRIBES.iter().position(|l| l == t))
                .map(|i| i as u32)
                .collect(),
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
                    hero_options: p.hero_options.clone(),
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Self {
            catalog: header.catalog.clone(),
            pool,
            shared_seed: header.shared_seed,
            players,
            meta: header.meta.clone(),
            duos: header.duos,
            first_fighter_switch_chance: if header.duos {
                header.first_fighter_switch_chance
            } else {
                crate::duos_tavern::DEFAULT_FIRST_FIGHTER_SWITCH_CHANCE
            },
            lobby_tribes: header
                .lobby_tribes
                .iter()
                .map(|&i| {
                    LOBBY_TRIBES
                        .get(i as usize)
                        .copied()
                        .ok_or_else(|| format!("invalid lobby type {i}"))
                })
                .collect::<Result<_, String>>()?,
        })
    }
}

/// A match between players that records every engine call into a replay (`docs/replays.md`).
///
/// Drive it like the bare engine, with [`Self::start_turn`], [`Self::step`] and [`Self::combat`]
/// (Duos: [`Self::duos_combat`]; a ghost: [`Self::ghost_combat`]) in whatever order the game loop
/// wants, and the lobby with [`Self::set_pairs`], [`Self::deliver_gifts`] and
/// [`Self::record_deaths`], then save [`Self::replay`]. Each call is recorded after its legality
/// check and before it runs, so the replay of a game that panicked ends with the call that
/// panicked. [`crate::lobby_game::LobbyGame`] drives a whole lobby this way.
///
/// Players are indexed `0..num_players()`, units `0..num_units()`; passing any other index panics.
#[derive(Clone, Debug)]
pub struct Match {
    header: pb::Header,
    pool_mode: PoolMode,
    /// Every player's Tavern. In Duos, a copy of the teams' players, refreshed after every call
    /// that can change them, so that players read the same in both modes.
    players: Vec<TavernState>,
    /// Duos: every team's Tavern (the source of truth for its players). Empty in Solo.
    teams: Vec<DuosTavern>,
    pools: Vec<CardPool>,
    rngs: Vec<Rng>,
    lobby: Lobby,
    records: Vec<pb::Record>,
}

/// The result of a combat: a Solo battle or a Duos team battle.
#[derive(Clone, Debug, PartialEq)]
pub enum CombatResult {
    Solo(Box<BattleResult>),
    Duos(Box<DuosResult>),
}

impl CombatResult {
    /// The outcome for side A.
    pub fn outcome(&self) -> BattleOutcome {
        match self {
            Self::Solo(r) => r.outcome,
            Self::Duos(r) => r.outcome,
        }
    }

    /// The damage the losing side takes.
    pub fn hero_damage(&self) -> u32 {
        match self {
            Self::Solo(r) => r.hero_damage,
            Self::Duos(r) => r.hero_damage,
        }
    }
}

impl Match {
    /// Start a match: every player at Turn 0 (call [`Self::start_turn`] to begin Turn 1).
    pub fn new(setup: &MatchSetup) -> Result<Self, String> {
        if setup.players.is_empty() {
            return Err("a match needs at least one player".to_string());
        }
        if setup.duos && !setup.players.len().is_multiple_of(2) {
            return Err(format!(
                "a Duos match has an even number of players, got {}",
                setup.players.len()
            ));
        }
        let mut templates = cards::catalog_for(&setup.catalog)?;
        if !setup.lobby_tribes.is_empty() {
            templates.retain(|t| in_lobby(t.tribe, &setup.lobby_tribes));
        }
        let mut players: Vec<TavernState> = setup
            .players
            .iter()
            .map(|p| {
                let mut state = TavernState::new().with_shop_spells(p.shop_spells);
                state.trinkets_enabled = p.trinkets;
                state.dark_gifts_enabled = p.dark_gifts;
                state.auras.deity.kind = p.deity;
                if !setup.duos {
                    match p.hero {
                        Some(id) => state
                            .set_hero(id)
                            .map_err(|e| format!("player {:?}: {e}", p.name))?,
                        None if !p.hero_options.is_empty() => {}
                        None => state.armor = p.armor,
                    }
                }
                if p.hero.is_none() && !p.hero_options.is_empty() {
                    state.offer_heroes(p.hero_options.clone());
                }
                Ok(state)
            })
            .collect::<Result<_, String>>()?;
        let mut teams = Vec::new();
        if setup.duos {
            for (t, pair) in setup.players.chunks(2).enumerate() {
                // Without a hero (or an offer), a player brings their `armor` to the team's pool.
                let armor = |p: &PlayerSetup| {
                    if p.hero.is_none() && p.hero_options.is_empty() {
                        p.armor
                    } else {
                        0
                    }
                };
                let mut team = DuosTavern::new()
                    .with_first_fighter_switch_chance(setup.first_fighter_switch_chance);
                team.players = [players[2 * t].clone(), players[2 * t + 1].clone()];
                team = team.with_armor([armor(&pair[0]), armor(&pair[1])]);
                team = team
                    .with_heroes([pair[0].hero, pair[1].hero])
                    .map_err(|e| format!("team {t}: {e}"))?;
                teams.push(team);
            }
            for (t, team) in teams.iter().enumerate() {
                players[2 * t] = team.players[0].clone();
                players[2 * t + 1] = team.players[1].clone();
            }
        }
        let units = if setup.duos {
            teams.len()
        } else {
            players.len()
        };
        let (pools, rngs) = match setup.pool {
            PoolMode::Shared => (
                vec![CardPool::new(templates)],
                vec![Rng::new(setup.shared_seed)],
            ),
            PoolMode::PerPlayer => {
                let step = if setup.duos { 2 } else { 1 };
                (
                    vec![CardPool::new(templates); units],
                    (0..units)
                        .map(|u| Rng::new(setup.players[u * step].seed))
                        .collect(),
                )
            }
        };
        Ok(Self {
            header: setup.to_header(),
            pool_mode: setup.pool,
            lobby: Lobby::new(players.len(), setup.duos),
            players,
            teams,
            pools,
            rngs,
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

    /// Whether this is a Duos match.
    pub fn is_duos(&self) -> bool {
        self.header.duos
    }

    /// The number of units: players in Solo, teams in Duos.
    pub fn num_units(&self) -> usize {
        if self.is_duos() {
            self.teams.len()
        } else {
            self.players.len()
        }
    }

    /// The unit player `p` belongs to: `p` in Solo, their team in Duos.
    pub fn unit_of(&self, p: usize) -> usize {
        if self.is_duos() {
            p / 2
        } else {
            p
        }
    }

    /// The players of unit `u`: `[u]` in Solo, `[2u, 2u + 1]` in Duos.
    pub fn unit_players(&self, u: usize) -> Vec<usize> {
        if self.is_duos() {
            vec![2 * u, 2 * u + 1]
        } else {
            vec![u]
        }
    }

    /// Unit `u`'s Health and Armor (a Duos team's shared pool).
    pub fn unit_health(&self, u: usize) -> (i32, i32) {
        if self.is_duos() {
            (self.teams[u].health, self.teams[u].armor)
        } else {
            (self.players[u].health, self.players[u].armor)
        }
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

    /// Duos: every team's Tavern (empty in Solo).
    pub fn teams(&self) -> &[DuosTavern] {
        &self.teams
    }

    /// The card pool player `p` draws from.
    pub fn pool(&self, p: usize) -> &CardPool {
        &self.pools[self.slot(self.unit_of(p))]
    }

    /// The RNG player `p` draws from.
    pub fn rng(&self, p: usize) -> &Rng {
        &self.rngs[self.slot(self.unit_of(p))]
    }

    /// Every card pool: one per unit ([`PoolMode::PerPlayer`]) or one shared.
    pub fn pools(&self) -> &[CardPool] {
        &self.pools
    }

    /// Every RNG: one per unit ([`PoolMode::PerPlayer`]) or one shared.
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

    /// Start unit `u`'s turn: [`TavernState::start_turn`] for player `u` (Duos:
    /// [`DuosTavern::start_turn`] for team `u`).
    pub fn start_turn(&mut self, u: usize) {
        self.check_unit(u);
        let first = self.unit_players(u)[0];
        let turn = self.players[first].turn + 1;
        self.records.push(record(Kind::StartTurn(pb::StartTurn {
            player: u as u32,
            turn,
        })));
        let s = self.slot(u);
        if self.is_duos() {
            self.teams[u].start_turn(&mut self.pools[s], &mut self.rngs[s]);
            self.sync_team(u);
        } else {
            self.players[u].start_turn(&mut self.pools[s], &mut self.rngs[s]);
        }
    }

    /// Whether player `p` may take `action` now ([`TavernState::is_legal`], [`DuosTavern::is_legal`]).
    pub fn is_legal(&self, p: usize, action: &TavernAction) -> bool {
        self.check_player(p);
        if self.is_duos() {
            self.teams[p / 2].is_legal(p % 2, action)
        } else {
            self.players[p].is_legal(action)
        }
    }

    /// Every action player `p` may take now.
    pub fn valid_actions(&self, p: usize) -> Vec<TavernAction> {
        self.check_player(p);
        if self.is_duos() {
            self.teams[p / 2].valid_actions(p % 2)
        } else {
            self.players[p].valid_actions()
        }
    }

    /// [`TavernState::step`] for player `p` (Duos: [`DuosTavern::step`]). Illegal actions return
    /// `Err` and are not recorded.
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
        if !self.is_legal(p, &action) {
            return Err(format!("illegal tavern action: {action:?}"));
        }
        let card_id = replay::action_card(&self.players[p], &action);
        self.records.push(record(Kind::Act(pb::Act {
            player: p as u32,
            action: Some(replay::action_to_pb(&action)),
            card_id,
            note,
        })));
        let u = self.unit_of(p);
        let s = self.slot(u);
        let result = if self.is_duos() {
            let result = self.teams[u].step(p % 2, action, &mut self.pools[s], &mut self.rngs[s]);
            self.sync_team(u);
            result
        } else {
            self.players[p].step(action, &mut self.pools[s], &mut self.rngs[s])
        };
        if result.is_err() {
            self.records.pop();
        }
        result
    }

    /// `EndTurn` for player `p`.
    pub fn end_turn(&mut self, p: usize) -> Result<bool, String> {
        self.step(p, TavernAction::EndTurn)
    }

    /// The coming turn's pairings of players: [`Lobby::set_next_pairs`], then [`Lobby::apply`]
    /// (every player's Tavern gets their view). Call it before the players start their turns. In
    /// Duos, pair each player with the opposing team's player in their seat.
    pub fn set_pairs(&mut self, pairs: &[(usize, usize)]) {
        self.set_pairs_with_damage_cap(pairs, None);
    }

    /// [`Self::set_pairs`], with the most combat damage a player (Duos: a team) can take this
    /// turn ([`Lobby::damage_cap`]; `None`: uncapped). Every player's view carries the cap, and
    /// the combats until the next call apply it.
    pub fn set_pairs_with_damage_cap(&mut self, pairs: &[(usize, usize)], damage_cap: Option<u32>) {
        for &(a, b) in pairs {
            self.check_player(a);
            self.check_player(b);
            assert_ne!(
                self.unit_of(a),
                self.unit_of(b),
                "players {a} and {b} cannot be paired"
            );
        }
        self.records.push(record(Kind::SetPairs(pb::SetPairs {
            pairs: pairs
                .iter()
                .map(|&(a, b)| pb::Pair {
                    a: a as u32,
                    b: b as u32,
                })
                .collect(),
            damage_cap,
        })));
        self.lobby.set_next_pairs(pairs);
        self.lobby.damage_cap = damage_cap;
        let lobby = std::mem::take(&mut self.lobby);
        self.with_states(|states| lobby.apply(states));
        self.lobby = lobby;
    }

    /// [`Lobby::deliver_gifts`]: every player's lobby gifts reach their living opponents.
    pub fn deliver_gifts(&mut self) {
        self.records
            .push(record(Kind::DeliverGifts(pb::DeliverGifts {})));
        let lobby = std::mem::take(&mut self.lobby);
        self.with_states(|states| lobby.deliver_gifts(states));
        self.lobby = lobby;
    }

    /// The combats are over: [`Lobby::record_deaths`] with every player whose unit is still above
    /// 0 Health alive. Returns who is alive, per player.
    pub fn record_deaths(&mut self) -> Vec<bool> {
        let alive: Vec<bool> = (0..self.players.len())
            .map(|p| self.unit_health(self.unit_of(p)).0 > 0)
            .collect();
        self.records
            .push(record(Kind::RecordDeaths(pb::RecordDeaths {
                alive: alive.clone(),
            })));
        self.lobby.record_deaths(&alive);
        alive
    }

    /// [`TavernState::resolve_combat_pair`] between Solo players `a` and `b`, with a seed drawn
    /// from the RNG player `a` draws from. The lobby records both players' combats and the outcome
    /// ([`Lobby::record_combat`], [`Lobby::record_outcome`]).
    pub fn combat(&mut self, a: usize, b: usize) -> BattleResult {
        assert!(!self.is_duos(), "a Duos match fights with `duos_combat`");
        match self.unit_combat(a, b, None) {
            CombatResult::Solo(r) => *r,
            CombatResult::Duos(_) => unreachable!("a Solo combat"),
        }
    }

    /// [`Self::combat`] with a seed chosen by the caller (no RNG draw).
    pub fn combat_with_seed(&mut self, a: usize, b: usize, seed: u64) -> BattleResult {
        assert!(!self.is_duos(), "a Duos match fights with `duos_combat`");
        match self.unit_combat(a, b, Some(seed)) {
            CombatResult::Solo(r) => *r,
            CombatResult::Duos(_) => unreachable!("a Solo combat"),
        }
    }

    /// [`DuosTavern::resolve_combat_pair`] between teams `a` and `b`, with a seed drawn from the
    /// RNG team `a` draws from. The lobby records every player's combat (against the opposing
    /// player in their seat) and the outcome.
    pub fn duos_combat(&mut self, a: usize, b: usize) -> DuosResult {
        assert!(self.is_duos(), "a Solo match fights with `combat`");
        match self.unit_combat(a, b, None) {
            CombatResult::Duos(r) => *r,
            CombatResult::Solo(_) => unreachable!("a Duos combat"),
        }
    }

    /// Units `a` and `b` fight: [`Self::combat`] or [`Self::duos_combat`], whichever the match
    /// plays, with the seed `seed` if given (else drawn from `a`'s RNG).
    pub fn unit_combat(&mut self, a: usize, b: usize, seed: Option<u64>) -> CombatResult {
        self.check_unit(a);
        self.check_unit(b);
        assert_ne!(a, b, "unit {a} cannot fight themselves");
        let external_seed = seed.is_some();
        let seed = seed.unwrap_or_else(|| {
            let s = self.slot(a);
            self.rngs[s].next_u64()
        });
        self.records.push(record(Kind::Combat(pb::Combat {
            a: a as u32,
            b: b as u32,
            seed,
            external_seed,
            ..Default::default()
        })));
        let (players_a, players_b) = (self.unit_players(a), self.unit_players(b));
        let boards_a: Vec<_> = players_a
            .iter()
            .map(|&p| self.players[p].board.clone())
            .collect();
        let boards_b: Vec<_> = players_b
            .iter()
            .map(|&p| self.players[p].board.clone())
            .collect();
        let result = if self.is_duos() {
            let (team_a, team_b) = pair_mut(&mut self.teams, a, b);
            let result = DuosTavern::resolve_combat_pair(team_a, team_b, seed);
            self.sync_team(a);
            self.sync_team(b);
            CombatResult::Duos(Box::new(result))
        } else {
            let (player_a, player_b) = pair_mut(&mut self.players, a, b);
            CombatResult::Solo(Box::new(TavernState::resolve_combat_pair(
                player_a, player_b, seed,
            )))
        };
        let result = self.cap_damage(result);
        for i in 0..players_a.len() {
            let (pa, pb_) = (players_a[i], players_b[i]);
            self.lobby.record_combat(pa, Some(pb_), &boards_a[i]);
            self.lobby.record_combat(pb_, Some(pa), &boards_b[i]);
            self.lobby.record_outcome(pa, pb_, result.outcome());
        }
        let (check_a, check_b) = (self.unit_checkpoint(a), self.unit_checkpoint(b));
        if let Some(pb::Record {
            kind: Some(Kind::Combat(combat)),
        }) = self.records.last_mut()
        {
            combat.set_outcome(replay::outcome_to_pb(result.outcome()));
            combat.hero_damage = result.hero_damage();
            combat.check_a = Some(check_a);
            combat.check_b = Some(check_b);
        }
        result
    }

    /// Unit `u` fights the ghost of unit `ghost`, who is out of the game: the boards `ghost`'s
    /// players entered their last combat with ([`Lobby::last_warbands`]), at their Tavern Tiers,
    /// with no hand or auras. The seed is drawn from `u`'s RNG; only `u` takes the result and the
    /// damage. The lobby records `u`'s players' combats (against the ghost's players).
    pub fn ghost_combat(&mut self, u: usize, ghost: usize) -> CombatResult {
        self.check_unit(u);
        self.check_unit(ghost);
        assert_ne!(u, ghost, "unit {u} cannot fight their own ghost");
        let s = self.slot(u);
        let seed = self.rngs[s].next_u64();
        self.records.push(record(Kind::GhostCombat(pb::GhostCombat {
            unit: u as u32,
            ghost: ghost as u32,
            seed,
            ..Default::default()
        })));
        let (players, ghosts) = (self.unit_players(u), self.unit_players(ghost));
        let boards: Vec<_> = players
            .iter()
            .map(|&p| self.players[p].board.clone())
            .collect();
        let result = if self.is_duos() {
            let warband = |g: usize| Warband {
                board: self.lobby.last_warbands[g].clone(),
                hand: Vec::new(),
                auras: PlayerAuras::default(),
                hero_tier: self.players[g].tavern_tier,
            };
            let opponent = DuosTeam {
                warbands: [warband(ghosts[0]), warband(ghosts[1])],
                first: 0,
            };
            let result = self.teams[u].resolve_combat_against(&opponent, seed);
            self.sync_team(u);
            CombatResult::Duos(Box::new(result))
        } else {
            let board = self.lobby.last_warbands[ghost].clone();
            let tier = self.players[ghost].tavern_tier;
            CombatResult::Solo(Box::new(self.players[u].resolve_combat_against(
                &board,
                tier,
                &PlayerAuras::default(),
                &[],
                seed,
            )))
        };
        let result = self.cap_damage(result);
        for i in 0..players.len() {
            self.lobby
                .record_combat(players[i], Some(ghosts[i]), &boards[i]);
        }
        let check = self.unit_checkpoint(u);
        if let Some(pb::Record {
            kind: Some(Kind::GhostCombat(combat)),
        }) = self.records.last_mut()
        {
            combat.set_outcome(replay::outcome_to_pb(result.outcome()));
            combat.hero_damage = result.hero_damage();
            combat.check = Some(check);
        }
        result
    }

    /// Record the end of the match and return its winner: the player with the most Health, or
    /// `None` if several players tie for the most. (Duos: the team with the most Health.)
    pub fn finish(&mut self) -> Option<usize> {
        let units = self.num_units();
        let best = (0..units).map(|u| self.unit_health(u).0).max().unwrap_or(0);
        let mut leaders = (0..units).filter(|&u| self.unit_health(u).0 == best);
        let winner = match (leaders.next(), leaders.next()) {
            (Some(u), None) => Some(u),
            _ => None,
        };
        self.push_end(winner, Vec::new());
        winner
    }

    /// Record the end of the match with each unit's final place (1 is first), as the game loop
    /// placed them. The winner is the unit in first place.
    pub fn finish_with_placements(&mut self, placements: &[u32]) -> Option<usize> {
        assert_eq!(
            placements.len(),
            self.num_units(),
            "one place per unit ({} units)",
            self.num_units()
        );
        let winner = placements.iter().position(|&p| p == 1);
        self.push_end(winner, placements.to_vec());
        winner
    }

    /// `result` with its hero damage limited to the damage the loser took: the lobby's damage
    /// cap ([`Lobby::damage_cap`]), which the Taverns applied.
    fn cap_damage(&self, mut result: CombatResult) -> CombatResult {
        let cap = self.lobby.damage_cap;
        match &mut result {
            CombatResult::Solo(r) => {
                r.hero_damage = crate::model::capped_damage(r.hero_damage, cap)
            }
            CombatResult::Duos(r) => {
                r.hero_damage = crate::model::capped_damage(r.hero_damage, cap)
            }
        }
        result
    }

    fn push_end(&mut self, winner: Option<usize>, placements: Vec<u32>) {
        self.records.push(record(Kind::End(pb::End {
            winner: winner.map(|u| u as u32),
            turn: self.players.iter().map(|s| s.turn).max().unwrap_or(0),
            health: self.players.iter().map(|s| s.health).collect(),
            placements,
        })));
    }

    /// A summary of unit `u`'s state: [`replay::checkpoint`] of a Solo player; for a Duos team,
    /// its Health and Armor, its first player's Tier, and both players' fingerprints combined.
    fn unit_checkpoint(&self, u: usize) -> pb::Checkpoint {
        if self.is_duos() {
            let team = &self.teams[u];
            pb::Checkpoint {
                health: team.health,
                armor: team.armor,
                tier: team.players[0].tavern_tier,
                fingerprint: replay::fingerprint(&team.players[0])
                    ^ replay::fingerprint(&team.players[1]).rotate_left(1),
            }
        } else {
            replay::checkpoint(&self.players[u])
        }
    }

    /// Run `f` on every player's Tavern (in Duos, the teams' players, which then settle into the
    /// team and are copied back).
    fn with_states(&mut self, f: impl FnOnce(&mut [&mut TavernState])) {
        if self.is_duos() {
            let mut states: Vec<&mut TavernState> = self
                .teams
                .iter_mut()
                .flat_map(|t| t.players.iter_mut())
                .collect();
            f(&mut states);
            for t in 0..self.teams.len() {
                self.teams[t].settle();
                self.sync_team(t);
            }
        } else {
            let mut states: Vec<&mut TavernState> = self.players.iter_mut().collect();
            f(&mut states);
        }
    }

    /// Copy team `t`'s players into [`Self::players`].
    fn sync_team(&mut self, t: usize) {
        let [a, b] = &self.teams[t].players;
        self.players[2 * t] = a.clone();
        self.players[2 * t + 1] = b.clone();
    }

    fn slot(&self, u: usize) -> usize {
        match self.pool_mode {
            PoolMode::PerPlayer => u,
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

    fn check_unit(&self, u: usize) {
        assert!(
            u < self.num_units(),
            "unit {u} does not exist (the match has {} units)",
            self.num_units()
        );
    }
}

/// Whether a minion of type `tribe` is in a lobby of the types `lobby`: it has no type, every
/// type, or one of them (a two-type minion through either of its types).
pub fn in_lobby(tribe: Tribe, lobby: &[Tribe]) -> bool {
    tribe == Tribe::None || tribe == Tribe::All || lobby.iter().any(|&t| tribe.has_single(t))
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
