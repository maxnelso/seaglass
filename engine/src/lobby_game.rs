//! Full lobbies (`docs/lobby_game.md`): 8 Solo players, or 4 Duos teams of 2, played from the hero
//! choice to the last unit standing.
//!
//! A [`LobbyGame`] drives a recorded [`Match`] with the rules the engine leaves to a single
//! Tavern's caller (`docs/tavern.md` §3.1, `docs/duos_tavern.md` §1):
//! - the hero offers (`heroes::draw_offers`),
//! - one shared card pool and RNG for the whole lobby,
//! - each turn's pairings, avoiding recent opponents,
//! - the ghost an odd one out fights (the last unit knocked out),
//! - the combats, the knockouts and the final placements.
//!
//! The players' decisions come from outside, one action at a time ([`LobbyGame::step`]). A game
//! goes through phases: [`LobbyGame::begin_phase`] starts one, every living player acts until they
//! are done ([`LobbyGame::active_players`]), and [`LobbyGame::end_phase`] resolves it. The phases
//! are the hero phase (with heroes on: every player picks a hero), then the turns (pairings, the
//! Tavern phase, then the combats and the knockouts).
//!
//! Units are what fights and gets placed: players in Solo, teams in Duos ([`Match`]). Pairings and
//! placements are the game loop's decisions, drawn from its own RNG; the match records them
//! (`SetPairs`, `Combat`, `GhostCombat`, `End`), so a replay re-simulates without it.

use std::collections::BTreeMap;

use crate::game::{CombatResult, Match, MatchSetup, PlayerSetup, PoolMode};
use crate::heroes;
use crate::model::{BattleOutcome, DeityKind, Tribe, Unit, LOBBY_TRIBES};
use crate::replay::pb;
use crate::rng::Rng;
use crate::tavern::{TavernAction, TavernState};

/// Salts mixed into the lobby's seed: its pairings' RNG, its hero offers.
const PAIRING_SALT: u64 = 0x5EA6_1A55_0B0B_0001;
const HERO_SALT: u64 = 0x5EA6_1A55_0B0B_0002;
const TRIBE_SALT: u64 = 0x5EA6_1A55_0B0B_0003;
/// The most actions [`LobbyGame::force_end`] takes to close a player's phase.
const MAX_FORCED_ACTIONS: usize = 64;
/// The random pairings [`LobbyGame`] tries each turn, keeping the one with the fewest repeats.
const PAIRING_TRIES: usize = 16;

/// The rules a [`LobbyGame`] plays by.
#[derive(Clone, Debug, PartialEq)]
pub struct LobbyConfig {
    pub duos: bool,
    /// Solo: 2..=8 players. Duos: 4, 6 or 8 (2..=4 teams).
    pub players: usize,
    /// After this turn's combats, the units still in are placed by Health plus Armor.
    pub max_turns: u32,
    /// Offer each player `hero_offers` heroes (`docs/heroes.md` §6). Without heroes, every player
    /// has 5 Armor (a Duos team 10) and no Hero Power.
    pub heroes: bool,
    pub hero_offers: usize,
    pub trinkets: bool,
    pub dark_gifts: bool,
    /// The card catalog; `None`: `"duos"` in Duos, `"full"` in Solo.
    pub catalog: Option<String>,
    pub deity: DeityKind,
    /// Duos: each team's chance of switching its first fighter each turn (`duos_tavern.md` §4.3).
    pub first_fighter_switch_chance: f64,
    /// Avoid pairing a unit with any of its last `avoid_recent` opponents, while possible.
    pub avoid_recent: usize,
    /// Cap combat damage as the game does (§5 of `docs/lobby_game.md`).
    pub damage_caps: bool,
    /// The number of minion types in the lobby, drawn at random from `LOBBY_TRIBES` (§1 of
    /// `docs/lobby_game.md`); 0 or 10: every type.
    pub tribes: usize,
    /// Seeds the shared card pool and RNG, the hero offers and the pairings.
    pub seed: u64,
    /// The players' names (default `p0`, `p1`, ...) and the replay's labels.
    pub names: Vec<String>,
    pub meta: BTreeMap<String, String>,
}

impl Default for LobbyConfig {
    fn default() -> Self {
        Self {
            duos: false,
            players: 8,
            max_turns: 30,
            heroes: true,
            hero_offers: 2,
            trinkets: true,
            dark_gifts: true,
            catalog: None,
            deity: DeityKind::None,
            first_fighter_switch_chance: crate::duos_tavern::DEFAULT_FIRST_FIGHTER_SWITCH_CHANCE,
            avoid_recent: 3,
            damage_caps: true,
            tribes: 5,
            seed: 0,
            names: Vec::new(),
            meta: BTreeMap::new(),
        }
    }
}

impl LobbyConfig {
    /// The number of units: players in Solo, teams in Duos.
    pub fn units(&self) -> usize {
        if self.duos {
            self.players / 2
        } else {
            self.players
        }
    }

    fn check(&self) -> Result<(), String> {
        if self.duos && (!self.players.is_multiple_of(2) || !(4..=8).contains(&self.players)) {
            return Err(format!(
                "a Duos lobby has 4, 6 or 8 players, got {}",
                self.players
            ));
        }
        if !self.duos && !(2..=8).contains(&self.players) {
            return Err(format!(
                "a Solo lobby has 2..=8 players, got {}",
                self.players
            ));
        }
        if self.max_turns == 0 {
            return Err("max_turns must be at least 1".to_string());
        }
        Ok(())
    }
}

/// Where a [`LobbyGame`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Every player picks one of their offered heroes (only `ChooseHero` is legal).
    Hero,
    /// A turn: every living player acts until they end their turn.
    Turn,
    /// Every unit is placed.
    Over,
}

/// What a player knows about one opponent: what the leaderboard shows (Health, Armor, Tavern
/// Tier, win streak), and the board they had the last time this player fought them, if ever. A
/// player never sees an opponent's board otherwise.
#[derive(Clone, Debug, PartialEq)]
pub struct OpponentInfo {
    pub player: usize,
    pub alive: bool,
    pub health: i32,
    pub armor: i32,
    pub tier: u32,
    /// Their hero (its id; 0 before they pick one): the leaderboard shows it.
    pub hero_id: u32,
    /// Their unit's current streak of combat wins.
    pub win_streak: u32,
    /// Their unit is this player's next opponent (or ghost).
    pub is_next: bool,
    /// Their unit was this player's last opponent.
    pub is_last: bool,
    /// The board they entered combat with the last time this player fought them (Duos: if their
    /// warband stepped in), and the turn of that combat. Empty and `None` if never seen.
    pub seen_board: Vec<Unit>,
    pub seen_turn: Option<u32>,
}

/// A player's view of their lobby, as of the start of the turn ([`LobbyGame::context`]).
#[derive(Clone, Debug, PartialEq)]
pub struct LobbyContext {
    pub duos: bool,
    pub units: usize,
    pub alive_units: usize,
    /// 1 + the number of living units with more Health plus Armor than the player's unit.
    pub health_rank: usize,
    /// The player's unit fights a ghost this turn.
    pub next_is_ghost: bool,
    /// The most combat damage the player's unit can take this turn, if capped (§5).
    pub damage_cap: Option<u32>,
    /// The minion types in the lobby (every player sees them from the start; empty: every type).
    pub tribes: Vec<Tribe>,
    /// The next opponent first (Duos: the opposing player in this seat, then their teammate),
    /// then the others: the living ones by Health plus Armor, then the ones who are out.
    pub opponents: Vec<OpponentInfo>,
}

/// One Solo or Duos lobby, played phase by phase (see the module doc).
#[derive(Clone, Debug)]
pub struct LobbyGame {
    config: LobbyConfig,
    game: Match,
    /// The game loop's own RNG: the pairings. Never an engine RNG.
    rng: Rng,
    phase: Phase,
    turn: u32,
    /// Each unit is still in.
    alive: Vec<bool>,
    /// Each unit's final place (1 is first), once it is out or the game is over.
    placement: Vec<Option<u32>>,
    /// Each unit's opponents, most recent first (`None`: a ghost or a bye).
    history: Vec<Vec<Option<usize>>>,
    /// The units knocked out, in order; the last one is the ghost.
    knocked_out: Vec<usize>,
    /// This turn's pairings (units), and the unit fighting the ghost.
    pairs: Vec<(usize, usize)>,
    ghost_fighter: Option<usize>,
    /// Each player is done with this phase (or out).
    done: Vec<bool>,
    /// What the leaderboard shows of each player: Health, Armor and Tavern Tier, refreshed at the
    /// start of each phase and after the combats.
    summary: Vec<(i32, i32, u32)>,
    /// Each unit's current streak of combat wins (the leaderboard shows it).
    win_streak: Vec<u32>,
    /// `seen[p][q]`: the board player `q` entered combat with the last time player `p` fought
    /// them, and that turn.
    seen: Vec<Vec<Option<SeenBoard>>>,
}

/// A board a player saw in combat, and the turn they saw it.
type SeenBoard = (Vec<Unit>, u32);

impl LobbyGame {
    /// A new lobby, before the hero phase (or, without heroes, before Turn 1).
    pub fn new(config: LobbyConfig) -> Result<Self, String> {
        config.check()?;
        let n = config.players;
        let offers = if config.heroes {
            heroes::draw_offers(
                n,
                config.hero_offers,
                config.duos,
                config.dark_gifts,
                &mut Rng::new(config.seed ^ HERO_SALT),
            )
        } else {
            vec![Vec::new(); n]
        };
        let players = (0..n)
            .map(|p| PlayerSetup {
                name: config
                    .names
                    .get(p)
                    .cloned()
                    .unwrap_or_else(|| format!("p{p}")),
                seed: config.seed.wrapping_add(p as u64),
                deity: config.deity,
                trinkets: config.trinkets,
                dark_gifts: config.dark_gifts,
                hero_options: offers[p].clone(),
                ..Default::default()
            })
            .collect();
        let lobby_tribes = draw_tribes(config.tribes, &mut Rng::new(config.seed ^ TRIBE_SALT));
        let mut meta = config.meta.clone();
        meta.entry("source".to_string())
            .or_insert_with(|| "LobbyGame".to_string());
        let game = Match::new(&MatchSetup {
            catalog: config
                .catalog
                .clone()
                .unwrap_or_else(|| if config.duos { "duos" } else { "full" }.to_string()),
            pool: PoolMode::Shared,
            shared_seed: config.seed,
            players,
            meta,
            duos: config.duos,
            first_fighter_switch_chance: config.first_fighter_switch_chance,
            lobby_tribes: lobby_tribes.clone(),
        })?;
        let units = config.units();
        let mut lobby = Self {
            rng: Rng::new(config.seed ^ PAIRING_SALT),
            phase: if config.heroes {
                Phase::Hero
            } else {
                Phase::Turn
            },
            turn: 0,
            alive: vec![true; units],
            placement: vec![None; units],
            history: vec![Vec::new(); units],
            knocked_out: Vec::new(),
            pairs: Vec::new(),
            ghost_fighter: None,
            done: vec![true; n],
            summary: vec![(0, 0, 1); n],
            win_streak: vec![0; units],
            seen: vec![vec![None; n]; n],
            config,
            game,
        };
        lobby.summarize();
        Ok(lobby)
    }

    // -- Accessors --------------------------------------------------------------------------------

    pub fn config(&self) -> &LobbyConfig {
        &self.config
    }

    /// The minion types in the lobby (`LOBBY_TRIBES` order; empty: every type).
    pub fn tribes(&self) -> Vec<Tribe> {
        let header = &self.game.header().lobby_tribes;
        header.iter().map(|&i| LOBBY_TRIBES[i as usize]).collect()
    }

    /// The recorded match ([`Match::save`] saves its replay).
    pub fn game(&self) -> &Match {
        &self.game
    }

    /// The replay's free-form labels.
    pub fn meta_mut(&mut self) -> &mut BTreeMap<String, String> {
        self.game.meta_mut()
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The current turn (0 before Turn 1).
    pub fn turn(&self) -> u32 {
        self.turn
    }

    pub fn is_over(&self) -> bool {
        self.phase == Phase::Over
    }

    pub fn num_players(&self) -> usize {
        self.config.players
    }

    pub fn num_units(&self) -> usize {
        self.config.units()
    }

    pub fn unit_of(&self, p: usize) -> usize {
        self.game.unit_of(p)
    }

    pub fn unit_players(&self, u: usize) -> Vec<usize> {
        self.game.unit_players(u)
    }

    /// Unit `u`'s Health and Armor.
    pub fn unit_health(&self, u: usize) -> (i32, i32) {
        self.game.unit_health(u)
    }

    /// Whether unit `u` is still in.
    pub fn is_alive(&self, u: usize) -> bool {
        self.alive[u]
    }

    /// Each unit's final place (1 is first), once it is out or the game is over.
    pub fn placements(&self) -> &[Option<u32>] {
        &self.placement
    }

    /// The units knocked out so far, in order (the game's survivors join them at the end).
    pub fn knocked_out(&self) -> &[usize] {
        &self.knocked_out
    }

    /// This turn's pairings (units) and the unit fighting the ghost, if any.
    pub fn pairs(&self) -> (&[(usize, usize)], Option<usize>) {
        (&self.pairs, self.ghost_fighter)
    }

    pub fn player(&self, p: usize) -> &TavernState {
        self.game.player(p)
    }

    /// Whether player `p` is done with this phase (or out).
    pub fn is_done(&self, p: usize) -> bool {
        self.done[p]
    }

    /// The players still to act in this phase.
    pub fn active_players(&self) -> Vec<usize> {
        if self.phase == Phase::Over {
            return Vec::new();
        }
        (0..self.config.players)
            .filter(|&p| !self.done[p])
            .collect()
    }

    pub fn is_legal(&self, p: usize, action: &TavernAction) -> bool {
        !self.done[p] && self.phase != Phase::Over && self.game.is_legal(p, action)
    }

    pub fn valid_actions(&self, p: usize) -> Vec<TavernAction> {
        if self.done[p] || self.phase == Phase::Over {
            return Vec::new();
        }
        self.game.valid_actions(p)
    }

    // -- Phases -----------------------------------------------------------------------------------

    /// Start the hero phase or the next turn. A turn starts with its pairings, every living unit's
    /// start of turn, and the lobby gifts.
    pub fn begin_phase(&mut self) -> Result<(), String> {
        match self.phase {
            Phase::Over => return Err("the game is over".to_string()),
            Phase::Hero => {
                for p in 0..self.config.players {
                    self.done[p] = self.game.player(p).hero_options.is_empty();
                }
                return Ok(());
            }
            Phase::Turn => {}
        }
        self.turn += 1;
        self.pair();
        let mut player_pairs = Vec::new();
        for &(a, b) in &self.pairs {
            player_pairs.extend(self.unit_players(a).into_iter().zip(self.unit_players(b)));
        }
        if let (Some(u), Some(&ghost)) = (self.ghost_fighter, self.knocked_out.last()) {
            // The ghost's players stand in as the next opponents (Scabbs and Tess read their
            // boards). Last, so the featured pair is a real one.
            player_pairs.extend(
                self.unit_players(u)
                    .into_iter()
                    .zip(self.unit_players(ghost)),
            );
        }
        let cap = self.damage_cap();
        self.game.set_pairs_with_damage_cap(&player_pairs, cap);
        for u in 0..self.num_units() {
            if self.alive[u] {
                self.game.start_turn(u);
            }
        }
        self.game.deliver_gifts();
        for p in 0..self.config.players {
            self.done[p] = !self.alive[self.unit_of(p)];
        }
        self.summarize();
        Ok(())
    }

    /// Player `p` takes `action` (with a note, recorded with it). Returns `true` once they are
    /// done with this phase: they ended their turn, or picked their hero.
    pub fn step(
        &mut self,
        p: usize,
        action: TavernAction,
        note: Option<pb::Note>,
    ) -> Result<bool, String> {
        if p >= self.config.players {
            return Err(format!(
                "player {p} does not exist (the lobby has {} players)",
                self.config.players
            ));
        }
        if self.phase == Phase::Over {
            return Err("the game is over".to_string());
        }
        if self.done[p] {
            return Err(format!("player {p} is done with this phase"));
        }
        let ended = self.game.step_with_note(p, action, note)?;
        self.done[p] = match self.phase {
            Phase::Hero => self.game.player(p).hero_options.is_empty(),
            _ => ended,
        };
        Ok(self.done[p])
    }

    /// End player `p`'s phase for them: pick their first hero, or resolve whatever prompt keeps
    /// their turn open (the first option of a Discover, a trinket offer, ...) and end it. Every
    /// action is recorded as forced (`Note::forced`).
    pub fn force_end(&mut self, p: usize) -> Result<(), String> {
        let forced = || {
            Some(pb::Note {
                forced: true,
                ..Default::default()
            })
        };
        for _ in 0..MAX_FORCED_ACTIONS {
            if self.done[p] || self.phase == Phase::Over {
                return Ok(());
            }
            let action = if self.game.is_legal(p, &TavernAction::EndTurn) {
                TavernAction::EndTurn
            } else {
                let actions = self.game.valid_actions(p);
                let prompt = actions.iter().find(|a| {
                    matches!(
                        a,
                        TavernAction::ChooseHero { .. }
                            | TavernAction::ChooseDiscover { .. }
                            | TavernAction::ChooseTrinket { .. }
                    )
                });
                match prompt.or(actions.first()) {
                    Some(&a) => a,
                    None => {
                        // Nothing is legal (a Duos player whose turn has ended): done.
                        self.done[p] = true;
                        return Ok(());
                    }
                }
            };
            self.step(p, action, forced())?;
        }
        Err(format!(
            "player {p}'s phase is still open after {MAX_FORCED_ACTIONS} forced actions"
        ))
    }

    /// Resolve the phase. After the hero phase, nothing happens. After a turn: the players still
    /// acting are forced to end it ([`Self::force_end`]), the lobby gifts go out, every pair and
    /// the ghost fight, and the units at 0 Health or less are knocked out and placed. If one unit
    /// (or none) is left, or the turn was the last ([`LobbyConfig::max_turns`]), every unit still
    /// in is placed and the game is over. Returns the units placed.
    pub fn end_phase(&mut self) -> Result<Vec<usize>, String> {
        match self.phase {
            Phase::Over => return Err("the game is over".to_string()),
            Phase::Hero => {
                for p in self.active_players() {
                    self.force_end(p)?;
                }
                self.phase = Phase::Turn;
                return Ok(Vec::new());
            }
            Phase::Turn => {}
        }
        for p in self.active_players() {
            self.force_end(p)?;
        }
        self.game.deliver_gifts();
        for (a, b) in self.pairs.clone() {
            self.history[a].insert(0, Some(b));
            self.history[b].insert(0, Some(a));
            let (players_a, players_b) = (self.unit_players(a), self.unit_players(b));
            let board = |g: &Self, p: usize| g.game.player(p).board.clone();
            let boards_a: Vec<Vec<Unit>> = players_a.iter().map(|&p| board(self, p)).collect();
            let boards_b: Vec<Vec<Unit>> = players_b.iter().map(|&p| board(self, p)).collect();
            let result = self.game.unit_combat(a, b, None);
            // Which warbands stepped in: every Solo board; a Duos warband only if it fought.
            let (fought_a, fought_b): (Vec<bool>, Vec<bool>) = match &result {
                CombatResult::Solo(_) => (vec![true], vec![true]),
                CombatResult::Duos(r) => (
                    r.team_a.iter().map(|w| w.fought).collect(),
                    r.team_b.iter().map(|w| w.fought).collect(),
                ),
            };
            let turn = self.turn;
            for &p in &players_a {
                for (i, &q) in players_b.iter().enumerate() {
                    if fought_b[i] {
                        self.seen[p][q] = Some((boards_b[i].clone(), turn));
                    }
                }
            }
            for &p in &players_b {
                for (i, &q) in players_a.iter().enumerate() {
                    if fought_a[i] {
                        self.seen[p][q] = Some((boards_a[i].clone(), turn));
                    }
                }
            }
            // A win extends the winner's streak; a loss or a draw ends it.
            let winner = match result.outcome() {
                BattleOutcome::AWin => Some(a),
                BattleOutcome::BWin => Some(b),
                BattleOutcome::Draw => None,
            };
            for u in [a, b] {
                self.win_streak[u] = if Some(u) == winner {
                    self.win_streak[u] + 1
                } else {
                    0
                };
            }
        }
        if let Some(u) = self.ghost_fighter {
            match self.knocked_out.last() {
                Some(&ghost) => {
                    self.history[u].insert(0, Some(ghost));
                    let result = self.game.ghost_combat(u, ghost);
                    match result.outcome() {
                        BattleOutcome::AWin => self.win_streak[u] += 1,
                        _ => self.win_streak[u] = 0,
                    }
                }
                None => self.history[u].insert(0, None),
            }
        }
        Ok(self.knockouts())
    }

    /// Place the units knocked out this turn (or, at the end of the game, every unit still in).
    fn knockouts(&mut self) -> Vec<usize> {
        let living: Vec<usize> = (0..self.num_units()).filter(|&u| self.alive[u]).collect();
        let mut out: Vec<usize> = living
            .iter()
            .copied()
            .filter(|&u| self.unit_health(u).0 <= 0)
            .collect();
        let survivors = living.len() - out.len();
        let game_over = survivors <= 1 || self.turn >= self.config.max_turns;
        if game_over {
            out = living.clone();
        }
        // The best of the units placed now takes the best place left: more Health plus Armor first
        // (a knockout's Health is 0 or less), then the lower unit.
        out.sort_by_key(|&u| {
            let (health, armor) = self.unit_health(u);
            (std::cmp::Reverse(health + armor), u)
        });
        let first = (living.len() - out.len() + 1) as u32;
        for (i, &u) in out.iter().enumerate() {
            self.placement[u] = Some(first + i as u32);
            self.alive[u] = false;
            self.knocked_out.push(u);
        }
        self.game.record_deaths();
        if game_over {
            let placements: Vec<u32> = self.placement.iter().map(|p| p.unwrap_or(0)).collect();
            self.game.finish_with_placements(&placements);
            self.phase = Phase::Over;
        }
        for p in 0..self.config.players {
            self.done[p] = true;
        }
        self.summarize();
        out
    }

    /// This turn's pairings: a random matching of the living units that avoids their recent
    /// opponents (the best of a few random tries) and, with an odd number, one fighter for the
    /// ghost (preferably not the one who fought it last turn).
    fn pair(&mut self) {
        let living: Vec<usize> = (0..self.num_units()).filter(|&u| self.alive[u]).collect();
        let mut best: Option<(usize, Vec<usize>)> = None;
        for _ in 0..PAIRING_TRIES {
            let mut order = living.clone();
            for i in (1..order.len()).rev() {
                let j = self.rng.below(i + 1);
                order.swap(i, j);
            }
            let mut cost = 0;
            if order.len() % 2 == 1 {
                let fighter = order[order.len() - 1];
                let ghost = self.knocked_out.last().copied();
                if ghost.is_some() && self.history[fighter].first() == Some(&ghost) {
                    cost += 2;
                }
            }
            for [a, b] in order.as_chunks::<2>().0 {
                let recent = &self.history[*a];
                let window = recent.len().min(self.config.avoid_recent);
                if let Some(i) = recent[..window].iter().position(|&o| o == Some(*b)) {
                    cost += self.config.avoid_recent - i;
                }
            }
            if best.as_ref().is_none_or(|(c, _)| cost < *c) {
                best = Some((cost, order));
            }
            if cost == 0 {
                break;
            }
        }
        let order = best.map(|(_, o)| o).unwrap_or_default();
        self.pairs = order
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&[a, b]| (a, b))
            .collect();
        self.ghost_fighter = (order.len() % 2 == 1).then(|| order[order.len() - 1]);
    }

    /// The most combat damage a unit can take this turn (with [`LobbyConfig::damage_caps`]): in
    /// Solo 5 on turns 1-3, 10 on turns 4-7 and 15 from turn 8, until only 4 players are left; in
    /// Duos 15 until only 2 teams are left. Then none.
    pub fn damage_cap(&self) -> Option<u32> {
        if !self.config.damage_caps {
            return None;
        }
        let living = self.alive.iter().filter(|&&a| a).count();
        if self.config.duos {
            (living > 2).then_some(15)
        } else if living <= 4 {
            None
        } else {
            Some(match self.turn {
                0..=3 => 5,
                4..=7 => 10,
                _ => 15,
            })
        }
    }

    /// Refresh what the leaderboard shows (the last values for a player who is out).
    fn summarize(&mut self) {
        for p in 0..self.config.players {
            let u = self.unit_of(p);
            if self.alive[u] || self.turn == 0 {
                let (health, armor) = self.unit_health(u);
                self.summary[p] = (health, armor, self.game.player(p).tavern_tier);
            }
        }
    }

    /// Player `p`'s view of the lobby ([`LobbyContext`]), as of the start of the turn.
    pub fn context(&self, p: usize) -> LobbyContext {
        let duos = self.config.duos;
        let unit = self.unit_of(p);
        let seat = p % 2;
        let total = |u: usize| {
            let (health, armor, _) = self.summary[self.unit_players(u)[0]];
            health + armor
        };
        let living: Vec<usize> = (0..self.num_units()).filter(|&u| self.alive[u]).collect();
        let health_rank = 1 + living
            .iter()
            .filter(|&&u| u != unit && total(u) > total(unit))
            .count();
        let mut next_unit = self.pairs.iter().find_map(|&(a, b)| {
            if a == unit {
                Some(b)
            } else if b == unit {
                Some(a)
            } else {
                None
            }
        });
        let next_is_ghost = self.ghost_fighter == Some(unit) && !self.knocked_out.is_empty();
        if next_is_ghost {
            next_unit = self.knocked_out.last().copied();
        }
        let last_unit = self.history[unit].first().copied().flatten();
        let mut ordered: Vec<usize> = Vec::new();
        if let Some(nu) = next_unit {
            let players = self.unit_players(nu);
            ordered = if duos {
                vec![players[seat], players[1 - seat]]
            } else {
                players
            };
        }
        let mut rest: Vec<usize> = (0..self.config.players)
            .filter(|&q| self.unit_of(q) != unit && !ordered.contains(&q))
            .collect();
        rest.sort_by_key(|&q| {
            let u = self.unit_of(q);
            (!self.alive[u], std::cmp::Reverse(total(u)), q)
        });
        let opponents = ordered
            .into_iter()
            .chain(rest)
            .map(|q| {
                let u = self.unit_of(q);
                let (health, armor, tier) = self.summary[q];
                let (seen_board, seen_turn) = match &self.seen[p][q] {
                    Some((board, turn)) => (board.clone(), Some(*turn)),
                    None => (Vec::new(), None),
                };
                OpponentInfo {
                    player: q,
                    alive: self.alive[u],
                    health,
                    armor,
                    tier,
                    hero_id: self.game.player(q).hero().map_or(0, |h| h.hero_id),
                    win_streak: self.win_streak[u],
                    is_next: Some(u) == next_unit,
                    is_last: Some(u) == last_unit,
                    seen_board,
                    seen_turn,
                }
            })
            .collect();
        LobbyContext {
            duos,
            units: self.num_units(),
            alive_units: living.len(),
            health_rank,
            next_is_ghost,
            damage_cap: self.game.lobby().damage_cap,
            tribes: self.tribes(),
            opponents,
        }
    }
}

/// `count` distinct lobby types, uniformly at random, in `LOBBY_TRIBES` order (none if `count` is
/// 0 or at least the number of types: every type).
fn draw_tribes(count: usize, rng: &mut Rng) -> Vec<Tribe> {
    if count == 0 || count >= LOBBY_TRIBES.len() {
        return Vec::new();
    }
    let mut left: Vec<usize> = (0..LOBBY_TRIBES.len()).collect();
    let mut picked: Vec<usize> = (0..count)
        .map(|_| left.remove(rng.below(left.len())))
        .collect();
    picked.sort_unstable();
    picked.into_iter().map(|i| LOBBY_TRIBES[i]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Play a lobby to the end with every player ending their turn at once.
    fn play_passive(config: LobbyConfig) -> LobbyGame {
        let mut game = LobbyGame::new(config).unwrap();
        while !game.is_over() {
            game.begin_phase().unwrap();
            game.end_phase().unwrap();
        }
        game
    }

    #[test]
    fn every_unit_is_placed_once() {
        for duos in [false, true] {
            let game = play_passive(LobbyConfig {
                duos,
                max_turns: 12,
                seed: 3,
                ..Default::default()
            });
            let mut places: Vec<u32> = game.placements().iter().map(|p| p.unwrap()).collect();
            places.sort();
            assert_eq!(places, (1..=game.num_units() as u32).collect::<Vec<_>>());
        }
    }

    #[test]
    fn pairings_avoid_the_last_opponent() {
        let mut game = LobbyGame::new(LobbyConfig {
            heroes: false,
            seed: 7,
            ..Default::default()
        })
        .unwrap();
        for _ in 0..4 {
            game.begin_phase().unwrap();
            for &(a, b) in &game.pairs.clone() {
                assert_ne!(
                    game.history[a].first(),
                    Some(&Some(b)),
                    "turn {}",
                    game.turn
                );
            }
            game.end_phase().unwrap();
        }
    }

    #[test]
    fn the_hero_phase_offers_heroes_and_force_end_picks_one() {
        let mut game = LobbyGame::new(LobbyConfig {
            players: 4,
            seed: 1,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(game.phase(), Phase::Hero);
        game.begin_phase().unwrap();
        assert_eq!(game.active_players(), vec![0, 1, 2, 3]);
        game.step(0, TavernAction::ChooseHero { option_index: 1 }, None)
            .unwrap();
        assert_eq!(game.active_players(), vec![1, 2, 3]);
        game.end_phase().unwrap();
        assert!((0..4).all(|p| game.player(p).auras.hero.is_some()));
        assert_eq!(game.phase(), Phase::Turn);
    }

    #[test]
    fn the_context_puts_the_next_opponent_first() {
        for duos in [false, true] {
            let mut game = LobbyGame::new(LobbyConfig {
                duos,
                seed: 9,
                heroes: false,
                ..Default::default()
            })
            .unwrap();
            game.begin_phase().unwrap();
            let ctx = game.context(0);
            assert_eq!(ctx.opponents.len(), if duos { 6 } else { 7 });
            assert!(ctx.opponents[0].is_next);
            if duos {
                assert!(ctx.opponents[1].is_next);
                assert_eq!(ctx.opponents[0].player % 2, 0);
            }
        }
    }
}
