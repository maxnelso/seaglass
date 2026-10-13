//! Card-agnostic combat resolution loop (`docs/combat.md`), and Duos combats built on it
//! (`docs/duos.md`).

use std::cmp::Ordering;
use std::collections::VecDeque;
use std::ops::Range;

use serde::Serialize;

use crate::cards::{self, BoardCtx, Passive, SocAction};
use crate::events::Event;
use crate::model::{BattleOutcome, DeityKind, GameState, PlayerAuras, Side, Tribe, Unit, UnitId};
use crate::rng::Rng;

/// Maximum number of units a board can hold at once.
pub const MAX_BOARD_SIZE: usize = 7;

/// Complete result of a single deterministic battle (`docs/combat.md` §5.6).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BattleResult {
    pub outcome: BattleOutcome,
    pub survivors_a: Vec<Unit>,
    pub survivors_b: Vec<Unit>,
    pub hero_damage: u32,
    pub events: Vec<Event>,
    pub hand_a: Vec<Unit>,
    pub hand_b: Vec<Unit>,
    pub auras_a: PlayerAuras,
    pub auras_b: PlayerAuras,
    pub dead_units_a: Vec<Unit>,
    pub dead_units_b: Vec<Unit>,
}

/// One player's warband in a Duos combat (`docs/duos.md` §1): their board, hand, player auras
/// and Tavern Tier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warband {
    pub board: Vec<Unit>,
    pub hand: Vec<Unit>,
    pub auras: PlayerAuras,
    /// The player's Tavern Tier, added to the damage if this warband wins (`docs/duos.md` §4).
    pub hero_tier: u32,
}

impl Default for Warband {
    fn default() -> Self {
        Self {
            board: Vec::new(),
            hand: Vec::new(),
            auras: PlayerAuras::default(),
            hero_tier: 1,
        }
    }
}

impl Warband {
    /// A warband with `board`, an empty hand, default auras and Tavern Tier 1.
    pub fn new(board: Vec<Unit>) -> Self {
        Self {
            board,
            ..Self::default()
        }
    }
}

/// One Duos team (`docs/duos.md` §1): its two players' warbands and the one that fights first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DuosTeam {
    pub warbands: [Warband; 2],
    /// The player (`0` or `1`) whose warband fights first.
    pub first: usize,
}

/// One player's result of a Duos combat (`docs/duos.md` §6), for their Tavern to apply.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WarbandResult {
    /// `true` if the warband stepped in. A warband that didn't comes back unchanged: its whole
    /// board as `survivors`, its hand and auras as given.
    pub fought: bool,
    pub survivors: Vec<Unit>,
    pub hand: Vec<Unit>,
    pub auras: PlayerAuras,
    pub dead_units: Vec<Unit>,
}

/// Complete result of a deterministic Duos combat (`docs/duos.md` §6).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DuosResult {
    /// The teams' outcome (`AWin`: team A won).
    pub outcome: BattleOutcome,
    /// The damage the losing team takes (`docs/duos.md` §4; 0 on a draw).
    pub hero_damage: u32,
    pub events: Vec<Event>,
    /// Team A's players' results: player 0, then player 1.
    pub team_a: [WarbandResult; 2],
    /// Team B's players' results: player 0, then player 1.
    pub team_b: [WarbandResult; 2],
}

/// Per-side combat state tracked during a single battle.
struct SideCombatState {
    board: Vec<Unit>,
    /// The starting warband (`docs/combat.md` §2).
    start_board: Vec<Unit>,
    ptr: Option<UnitId>,
    hero_tier: u32,
    auras: PlayerAuras,
    hand: Vec<Unit>,
    hand_summoned: Vec<bool>,
    hand_given_by: Vec<Option<UnitId>>,
    combat_beast_bonus_atk: i32,
    aberration_deaths: u32,
    deity_awakened: bool,
    dead_aberrations: Vec<Unit>,
    dead_units: Vec<Unit>,
    pending_immediate_attacks: Vec<UnitId>,
}

impl SideCombatState {
    fn new(mut board: Vec<Unit>, hero_tier: u32, auras: PlayerAuras, mut hand: Vec<Unit>) -> Self {
        for u in &mut board {
            if let Some(combat_start) = cards::hooks(u.card_id).combat_start {
                combat_start(u);
            }
            cards::sync_unit_auras(u, &auras);
            u.sync_max_stats();
            cards::check_stat_thresholds(u);
        }
        for h in &mut hand {
            cards::sync_unit_auras(h, &auras);
        }
        let hand_summoned = vec![false; hand.len()];
        let hand_given_by = vec![None; hand.len()];
        let ptr = board.first().map(|u| u.id);
        Self {
            start_board: board.clone(),
            board,
            ptr,
            hero_tier,
            auras,
            hand,
            hand_summoned,
            hand_given_by,
            combat_beast_bonus_atk: 0,
            aberration_deaths: 0,
            deity_awakened: false,
            dead_aberrations: Vec::new(),
            dead_units: Vec::new(),
            pending_immediate_attacks: Vec::new(),
        }
    }

    /// Borrow this side as a [`BoardCtx`] for card hooks.
    fn ctx<'a>(
        &'a mut self,
        side: Side,
        next_id: &'a mut UnitId,
        rng: &'a mut Rng,
        events: &'a mut Vec<Event>,
    ) -> BoardCtx<'a> {
        BoardCtx {
            side,
            in_combat: true,
            board: &mut self.board,
            start_board: &self.start_board,
            cursor: 0,
            reserved_slots: 0,
            auras: &mut self.auras,
            hand: &mut self.hand,
            hand_summoned: &mut self.hand_summoned,
            hand_given_by: &mut self.hand_given_by,
            dead_aberrations: &self.dead_aberrations,
            beast_bonus_atk: &mut self.combat_beast_bonus_atk,
            hero_tier: self.hero_tier,
            pending_attacks: &mut self.pending_immediate_attacks,
            destroys: Vec::new(),
            next_id,
            rng,
            events,
        }
    }

    fn prepare_summoned_unit(&mut self, token: &mut Unit) {
        cards::apply_combat_summon_modifiers(
            &mut self.board,
            &self.auras,
            self.combat_beast_bonus_atk,
            token,
        );
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.board.is_empty()
    }

    #[inline]
    fn can_attack(&self) -> bool {
        self.board.iter().any(|u| u.attack > 0)
    }

    /// `true` if this side has a Start-of-Combat effect: a minion's, a player effect's or a hand
    /// card's (`docs/combat.md` §5.1.1).
    fn has_start_of_combat(&self) -> bool {
        self.board.iter().any(cards::has_start_of_combat)
            || cards::has_player_start_of_combat(&self.hand, &self.auras)
    }
}

/// Resolve one deterministic battle from `(board_a, board_b, state, seed)`.
pub fn resolve_battle(
    board_a: &[Unit],
    board_b: &[Unit],
    state: &GameState,
    seed: u64,
) -> BattleResult {
    let mut next_id: UnitId = 0;
    let units_a = with_combat_ids(board_a, &mut next_id);
    let units_b = with_combat_ids(board_b, &mut next_id);

    let mut rng = Rng::new(seed);

    // `first_attacker` is decided after Start of Combat (docs/combat.md §5.1 step 6).
    let mut events = vec![Event::BattleStart {
        seed,
        first_attacker: Side::A,
    }];

    let mut side_a = SideCombatState::new(
        units_a,
        state.hero_tier_a,
        state.auras_a.clone(),
        state.hand_a.clone(),
    );
    let mut side_b = SideCombatState::new(
        units_b,
        state.hero_tier_b,
        state.auras_b.clone(),
        state.hand_b.clone(),
    );

    resolve_start_of_combat(
        &mut side_a,
        &mut side_b,
        &mut next_id,
        &mut rng,
        &mut events,
    );

    let first_attacker = side_with_more_minions(side_a.board.len(), side_b.board.len(), &mut rng);
    events[0] = Event::BattleStart {
        seed,
        first_attacker,
    };

    let stalled_draw = run_attack_turns(
        first_attacker,
        &mut side_a,
        &mut side_b,
        &mut next_id,
        &mut rng,
        &mut events,
    );

    let outcome = if stalled_draw || (side_a.is_empty() && side_b.is_empty()) {
        BattleOutcome::Draw
    } else if side_b.is_empty() {
        BattleOutcome::AWin
    } else {
        BattleOutcome::BWin
    };

    let hero_damage = match outcome {
        BattleOutcome::AWin => {
            state.hero_tier_a + side_a.board.iter().map(|u| u.tavern_tier).sum::<u32>()
        }
        BattleOutcome::BWin => {
            state.hero_tier_b + side_b.board.iter().map(|u| u.tavern_tier).sum::<u32>()
        }
        BattleOutcome::Draw => 0,
    };

    events.push(Event::BattleEnd {
        outcome,
        hero_damage,
    });

    BattleResult {
        outcome,
        survivors_a: side_a.board,
        survivors_b: side_b.board,
        hero_damage,
        events,
        hand_a: side_a.hand,
        hand_b: side_b.hand,
        auras_a: side_a.auras,
        auras_b: side_b.auras,
        dead_units_a: side_a.dead_units,
        dead_units_b: side_b.dead_units,
    }
}

/// `board`'s units with combat ids from `next_id` on, in board order, cut to the board cap
/// (`docs/combat.md` §5.1 step 1).
fn with_combat_ids(board: &[Unit], next_id: &mut UnitId) -> Vec<Unit> {
    let mut units: Vec<Unit> = board
        .iter()
        .cloned()
        .map(|mut u| {
            u.id = *next_id;
            *next_id += 1;
            u
        })
        .collect();
    units.truncate(MAX_BOARD_SIZE);
    units
}

/// Run attack turns, `current` first, until a side has no minions left (`docs/combat.md` §5.2).
/// Returns `true` if the combat stalled instead: neither side can attack.
fn run_attack_turns(
    mut current: Side,
    side_a: &mut SideCombatState,
    side_b: &mut SideCombatState,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) -> bool {
    while !side_a.is_empty() && !side_b.is_empty() {
        let (atk_side, def_side) = sides_mut(current, side_a, side_b);
        if atk_side.can_attack() {
            perform_attack_turn(current, atk_side, def_side, next_id, rng, events);
            current = current.other();
        } else if !def_side.can_attack() {
            return true;
        } else {
            events.push(Event::TurnSkipped { side: current });
            current = current.other();
        }
    }
    false
}

/// One team during a Duos combat (`docs/duos.md`): each player's warband, the one on the team's
/// side, and the ones that stepped in.
struct DuosTeamState {
    /// Each player's warband as given, with its combat ids (the result of a warband that never
    /// fights, and the teammate's minions that fill out the board for the damage).
    given: [Warband; 2],
    /// Each player's combat state (a waiting warband's never changes: no effect reaches it).
    warbands: [SideCombatState; 2],
    /// The player whose warband fights first.
    first: usize,
    /// The player whose warband is on the team's side: the fighting one (the first warband
    /// while none has stepped in, for a team whose two warbands are empty).
    fighting: usize,
    /// Which players' warbands have stepped in.
    fought: [bool; 2],
}

impl DuosTeamState {
    fn new(team: &DuosTeam, boards: [Vec<Unit>; 2]) -> Self {
        let [board_0, board_1] = boards;
        let with_board = |w: &Warband, board: Vec<Unit>| Warband {
            board,
            hand: w.hand.clone(),
            auras: w.auras.clone(),
            hero_tier: w.hero_tier,
        };
        let given = [
            with_board(&team.warbands[0], board_0),
            with_board(&team.warbands[1], board_1),
        ];
        let warbands = given.each_ref().map(|w| {
            SideCombatState::new(
                w.board.clone(),
                w.hero_tier,
                w.auras.clone(),
                w.hand.clone(),
            )
        });
        Self {
            given,
            warbands,
            first: team.first,
            fighting: team.first,
            fought: [false; 2],
        }
    }

    /// The fighting warband's combat state.
    fn side(&mut self) -> &mut SideCombatState {
        &mut self.warbands[self.fighting]
    }

    /// `true` if the team's fighting warband has no minions left.
    fn is_empty(&self) -> bool {
        self.warbands[self.fighting].is_empty()
    }

    /// The player whose warband opens the fight: the first warband, or its teammate if it is
    /// empty (`docs/duos.md` §3.1 step 5); `None` if both are empty.
    fn opening_player(&self) -> Option<usize> {
        [self.first, 1 - self.first]
            .into_iter()
            .find(|&p| !self.given[p].board.is_empty())
    }

    /// The teammate whose warband steps in once the fighting one has no minions left: one that
    /// hasn't fought yet and isn't empty (`docs/duos.md` §3.2, §3.5).
    fn waiting_teammate(&self) -> Option<usize> {
        let mate = 1 - self.fighting;
        (!self.fought[mate] && !self.given[mate].board.is_empty()).then_some(mate)
    }

    /// Put `player`'s warband on the team's side (its pointer is still on its leftmost minion)
    /// and log it; its `first_attacker` is recorded once its Start of Combat has resolved.
    fn step_in(&mut self, team: Side, player: usize, events: &mut Vec<Event>) {
        self.fighting = player;
        self.fought[player] = true;
        events.push(Event::WarbandSteppedIn {
            team,
            player,
            first_attacker: Side::A,
        });
    }

    /// The damage the team deals if it won (`docs/duos.md` §4): the fighting warband's player's
    /// Tavern Tier plus the tiers of its survivors, then of its teammate's minions if the
    /// teammate never fought, up to the board cap.
    fn damage(&self) -> u32 {
        let winner = &self.warbands[self.fighting];
        let mate = 1 - self.fighting;
        let fill_out: &[Unit] = if self.fought[mate] {
            &[]
        } else {
            &self.given[mate].board
        };
        let tiers: u32 = winner
            .board
            .iter()
            .chain(fill_out)
            .take(MAX_BOARD_SIZE)
            .map(|u| u.tavern_tier)
            .sum();
        winner.hero_tier + tiers
    }

    /// Each player's result (`docs/duos.md` §6).
    fn results(self) -> [WarbandResult; 2] {
        let [given_0, given_1] = self.given;
        let [side_0, side_1] = self.warbands;
        let result = |fought: bool, given: Warband, side: SideCombatState| {
            if fought {
                WarbandResult {
                    fought,
                    survivors: side.board,
                    hand: side.hand,
                    auras: side.auras,
                    dead_units: side.dead_units,
                }
            } else {
                WarbandResult {
                    fought,
                    survivors: given.board,
                    hand: given.hand,
                    auras: given.auras,
                    dead_units: Vec::new(),
                }
            }
        };
        [
            result(self.fought[0], given_0, side_0),
            result(self.fought[1], given_1, side_1),
        ]
    }
}

/// Resolve one deterministic Duos combat from `(team_a, team_b, seed)` (`docs/duos.md`): the
/// teams' warbands fight as a relay, each with the Solo rules and its own player context.
pub fn resolve_duos_battle(team_a: &DuosTeam, team_b: &DuosTeam, seed: u64) -> DuosResult {
    for team in [team_a, team_b] {
        assert!(
            team.first < 2,
            "a Duos team's first warband is player 0 or 1, got {}",
            team.first
        );
    }
    // Ids: the first warbands, A's then B's, then the second warbands (docs/duos.md §3.1 step 1).
    let mut next_id: UnitId = 0;
    let mut boards: [[Vec<Unit>; 2]; 2] = Default::default();
    for nth in 0..2 {
        for (t, team) in [team_a, team_b].into_iter().enumerate() {
            let player = if nth == 0 { team.first } else { 1 - team.first };
            boards[t][player] = with_combat_ids(&team.warbands[player].board, &mut next_id);
        }
    }
    let [boards_a, boards_b] = boards;
    let mut teams = [
        DuosTeamState::new(team_a, boards_a),
        DuosTeamState::new(team_b, boards_b),
    ];

    let mut rng = Rng::new(seed);
    // `first_attacker` is decided after Start of Combat (docs/duos.md §3.3).
    let mut events = vec![Event::BattleStart {
        seed,
        first_attacker: Side::A,
    }];

    // The opening warbands step in and their Start of Combat resolves together (§3.1 step 5).
    let mut entered = vec![0];
    let mut opening = Vec::new();
    for (side, team) in [Side::A, Side::B].into_iter().zip(teams.iter_mut()) {
        if let Some(player) = team.opening_player() {
            entered.push(events.len());
            team.step_in(side, player, &mut events);
            opening.push(side);
        }
    }
    {
        let [team_a, team_b] = &mut teams;
        resolve_entering_start_of_combat(
            &opening,
            team_a.side(),
            team_b.side(),
            &mut next_id,
            &mut rng,
            &mut events,
        );
    }
    entered.extend(step_in_duos_warbands(
        &mut teams,
        &mut next_id,
        &mut rng,
        &mut events,
    ));

    let mut stalled_draw = false;
    loop {
        let [team_a, team_b] = &mut teams;
        let first_attacker = side_with_more_minions(
            team_a.side().board.len(),
            team_b.side().board.len(),
            &mut rng,
        );
        record_first_attacker(&mut events, &entered, first_attacker);
        if team_a.is_empty() || team_b.is_empty() {
            break;
        }
        // Stall: a draw, even with teammates waiting (docs/duos.md §3.5).
        stalled_draw = run_attack_turns(
            first_attacker,
            team_a.side(),
            team_b.side(),
            &mut next_id,
            &mut rng,
            &mut events,
        );
        if stalled_draw {
            break;
        }
        entered = step_in_duos_warbands(&mut teams, &mut next_id, &mut rng, &mut events);
        if entered.is_empty() {
            break;
        }
    }

    // A team is out when its fighting warband has no minions left: no teammate could step in.
    let [team_a, team_b] = teams;
    let outcome = if stalled_draw || (team_a.is_empty() && team_b.is_empty()) {
        BattleOutcome::Draw
    } else if team_b.is_empty() {
        BattleOutcome::AWin
    } else {
        BattleOutcome::BWin
    };
    let hero_damage = match outcome {
        BattleOutcome::AWin => team_a.damage(),
        BattleOutcome::BWin => team_b.damage(),
        BattleOutcome::Draw => 0,
    };
    events.push(Event::BattleEnd {
        outcome,
        hero_damage,
    });

    DuosResult {
        outcome,
        hero_damage,
        events,
        team_a: team_a.results(),
        team_b: team_b.results(),
    }
}

/// Step in the waiting warband of each team whose fighting warband has no minions left, then
/// resolve their Start of Combat; again while a Start of Combat leaves a fighting warband with no
/// minions and a teammate waiting (`docs/duos.md` §3.2). Returns the indices of the
/// `WarbandSteppedIn` events, whose `first_attacker` the caller records.
fn step_in_duos_warbands(
    teams: &mut [DuosTeamState; 2],
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) -> Vec<usize> {
    let mut entered = Vec::new();
    loop {
        let mut stepping = Vec::new();
        for (side, team) in [Side::A, Side::B].into_iter().zip(teams.iter_mut()) {
            if !team.is_empty() {
                continue;
            }
            if let Some(player) = team.waiting_teammate() {
                entered.push(events.len());
                team.step_in(side, player, events);
                stepping.push(side);
            }
        }
        if stepping.is_empty() {
            return entered;
        }
        let [team_a, team_b] = teams;
        resolve_entering_start_of_combat(
            &stepping,
            team_a.side(),
            team_b.side(),
            next_id,
            rng,
            events,
        );
    }
}

/// Record `side` as the `first_attacker` of the `BattleStart` / `WarbandSteppedIn` events at
/// `indices`.
fn record_first_attacker(events: &mut [Event], indices: &[usize], side: Side) {
    for &i in indices {
        if let Event::BattleStart { first_attacker, .. }
        | Event::WarbandSteppedIn { first_attacker, .. } = &mut events[i]
        {
            *first_attacker = side;
        }
    }
}

/// The side with more minions, or a coin flip on a tie (`docs/combat.md` §4.1, §4.4).
fn side_with_more_minions(len_a: usize, len_b: usize, rng: &mut Rng) -> Side {
    match len_a.cmp(&len_b) {
        Ordering::Greater => Side::A,
        Ordering::Less => Side::B,
        Ordering::Equal if rng.below(2) == 0 => Side::A,
        Ordering::Equal => Side::B,
    }
}

/// `(own, opponent)`: the combat state of `side` and of the other side.
fn sides_mut<'s>(
    side: Side,
    side_a: &'s mut SideCombatState,
    side_b: &'s mut SideCombatState,
) -> (&'s mut SideCombatState, &'s mut SideCombatState) {
    match side {
        Side::A => (side_a, side_b),
        Side::B => (side_b, side_a),
    }
}

/// Resolve every Start-of-Combat effect (`docs/combat.md` §5.1.1): each player's own effects
/// and Start-of-Combat summons, then the minions' triggers, one minion per side in turn.
fn resolve_start_of_combat(
    side_a: &mut SideCombatState,
    side_b: &mut SideCombatState,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    // The side with more minions goes first; on a tie the order only needs a coin flip if both
    // sides have something to resolve.
    let (len_a, len_b) = (side_a.board.len(), side_b.board.len());
    let first = if len_a == len_b && !(side_a.has_start_of_combat() && side_b.has_start_of_combat())
    {
        Side::A
    } else {
        side_with_more_minions(len_a, len_b, rng)
    };
    resolve_start_of_combat_in_order(
        &[first, first.other()],
        side_a,
        side_b,
        next_id,
        rng,
        events,
    );
}

/// Resolve the Start of Combat of the Duos warbands that just entered the fight on `sides`
/// (`docs/duos.md` §3.1 step 5, §3.2): both sides' together, as at the start of a Solo battle,
/// or one side's alone (the other side's effects don't trigger again).
fn resolve_entering_start_of_combat(
    sides: &[Side],
    side_a: &mut SideCombatState,
    side_b: &mut SideCombatState,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    if sides.len() == 2 {
        resolve_start_of_combat(side_a, side_b, next_id, rng, events);
    } else {
        resolve_start_of_combat_in_order(sides, side_a, side_b, next_id, rng, events);
    }
}

/// Resolve the Start-of-Combat effects of the sides in `order` (`docs/combat.md` §5.1.1 steps
/// 2-4): each one's own effects and Start-of-Combat summons, in that order, then their minions'
/// triggers, one minion per side in turn, `order[0]`'s first.
fn resolve_start_of_combat_in_order(
    order: &[Side],
    side_a: &mut SideCombatState,
    side_b: &mut SideCombatState,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    let Some(&first) = order.first() else {
        return;
    };
    for &side in order {
        let (own_side, opp_side) = sides_mut(side, side_a, side_b);
        resolve_start_of_combat_effects(side, own_side, opp_side, rng, events);
        cards::start_of_combat_summons(&mut own_side.ctx(side, next_id, rng, events));
        resolve_effect_deaths(side, own_side, opp_side, next_id, rng, events);
    }

    let listed = |side: Side, board: &[Unit]| {
        if order.contains(&side) {
            start_of_combat_minions(board)
        } else {
            VecDeque::new()
        }
    };
    let mut queues = [
        listed(Side::A, &side_a.board),
        listed(Side::B, &side_b.board),
    ];
    let mut turn = first;
    while let Some((side, src_id)) = next_start_of_combat_minion(turn, &mut queues, side_a, side_b)
    {
        let (own_side, opp_side) = sides_mut(side, side_a, side_b);
        resolve_minion_start_of_combat(side, src_id, own_side, opp_side, next_id, rng, events);
        turn = side.other();
    }
}

/// The minions on `board` with a Start-of-Combat trigger, left to right.
fn start_of_combat_minions(board: &[Unit]) -> VecDeque<UnitId> {
    board
        .iter()
        .filter(|u| cards::has_start_of_combat(u))
        .map(|u| u.id)
        .collect()
}

/// The next minion to resolve its Start-of-Combat trigger: the next minion listed in `queues`
/// (A's, B's) for `turn` that is still alive, else the other side's (which then continues).
fn next_start_of_combat_minion(
    turn: Side,
    queues: &mut [VecDeque<UnitId>; 2],
    side_a: &SideCombatState,
    side_b: &SideCombatState,
) -> Option<(Side, UnitId)> {
    for side in [turn, turn.other()] {
        let (queue, board) = match side {
            Side::A => (&mut queues[0], &side_a.board),
            Side::B => (&mut queues[1], &side_b.board),
        };
        while let Some(id) = queue.pop_front() {
            if board.iter().any(|u| u.id == id && u.health > 0) {
                return Some((side, id));
            }
        }
    }
    None
}

/// Resolve the Start-of-Combat trigger of `side`'s minion `src_id` (`docs/combat.md` §5.1.1
/// step 3): its `start_of_combat` hook, then its [`SocAction`], resolving deaths after each.
fn resolve_minion_start_of_combat(
    side: Side,
    src_id: UnitId,
    own_side: &mut SideCombatState,
    opp_side: &mut SideCombatState,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    cards::on_minion_start_of_combat(&mut own_side.ctx(side, next_id, rng, events), src_id);
    resolve_effect_deaths(side, own_side, opp_side, next_id, rng, events);
    let action = own_side
        .board
        .iter()
        .find(|u| u.id == src_id && u.health > 0)
        .and_then(cards::start_of_combat_action);
    match action {
        Some(SocAction::DestroyNeighbors { both_sides }) => resolve_start_of_combat_destroys(
            side, src_id, both_sides, own_side, opp_side, next_id, rng, events,
        ),
        Some(SocAction::DamageAll { amount, waves }) => resolve_start_of_combat_aoe(
            side, src_id, amount, waves, own_side, opp_side, next_id, rng, events,
        ),
        Some(SocAction::Attack { times }) => resolve_start_of_combat_attacks(
            side, src_id, times, own_side, opp_side, next_id, rng, events,
        ),
        None => {}
    }
}

/// Emit and resolve the deaths an effect of `side` caused (a Start-of-Combat effect or a Rally),
/// the other side's first, then any queued immediate attacks (`docs/combat.md` §5.1.1 step 4).
fn resolve_effect_deaths(
    side: Side,
    own_side: &mut SideCombatState,
    opp_side: &mut SideCombatState,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    if !opp_side
        .board
        .iter()
        .chain(&own_side.board)
        .any(|u| u.health <= 0)
    {
        return;
    }
    for u in opp_side.board.iter().chain(&own_side.board) {
        if u.health <= 0 {
            events.push(Event::Death { unit: u.id });
        }
    }
    resolve_deaths(side.other(), opp_side, own_side, next_id, rng, events);
    resolve_deaths(side, own_side, opp_side, next_id, rng, events);
    resolve_pending_immediate_attacks(side, own_side, opp_side, next_id, rng, events);
}

/// Resolve the player's own Start-of-Combat effects (recorded in the Tavern, e.g. by spells),
/// before any minion's Start-of-Combat trigger.
fn resolve_start_of_combat_effects(
    side: Side,
    own_side: &mut SideCombatState,
    opp_side: &mut SideCombatState,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    cards::on_player_start_of_combat(
        side,
        &mut own_side.board,
        &mut own_side.auras,
        &mut opp_side.board,
        rng,
        events,
    );
}

/// Resolve `src_id`'s Start-of-Combat friendly neighbor destroys
/// ([`SocAction::DestroyNeighbors`]).
#[allow(clippy::too_many_arguments)]
fn resolve_start_of_combat_destroys(
    side: Side,
    src_id: UnitId,
    both_sides: bool,
    own_side: &mut SideCombatState,
    opp_side: &mut SideCombatState,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    let Some(src_pos) = own_side
        .board
        .iter()
        .position(|u| u.id == src_id && u.health > 0)
    else {
        return;
    };
    let src_card = own_side.board[src_pos].card_id;
    let mut victim_positions = Vec::new();
    if src_pos > 0
        && own_side.board[src_pos - 1].health > 0
        && own_side.board[src_pos - 1].card_id != src_card
    {
        victim_positions.push(src_pos - 1);
    }
    if both_sides
        && src_pos + 1 < own_side.board.len()
        && own_side.board[src_pos + 1].health > 0
        && own_side.board[src_pos + 1].card_id != src_card
    {
        victim_positions.push(src_pos + 1);
    }
    if victim_positions.is_empty() {
        return;
    }
    let mut stored_copies = Vec::new();
    for &v_pos in &victim_positions {
        let mut copy = own_side.board[v_pos].clone();
        copy.health = copy.max_health.max(copy.health);
        stored_copies.push(copy);
        own_side.board[v_pos].health = 0;
        events.push(Event::Death {
            unit: own_side.board[v_pos].id,
        });
    }
    own_side.board[src_pos]
        .stitched_stored
        .extend(stored_copies);
    resolve_deaths(side, own_side, opp_side, next_id, rng, events);
    resolve_pending_immediate_attacks(side, own_side, opp_side, next_id, rng, events);
}

/// Resolve `src_id`'s Start-of-Combat board-wide damage ([`SocAction::DamageAll`]), resolving
/// the deaths of each wave before the next.
#[allow(clippy::too_many_arguments)]
fn resolve_start_of_combat_aoe(
    side: Side,
    src_id: UnitId,
    amount: i32,
    waves: u32,
    own_side: &mut SideCombatState,
    opp_side: &mut SideCombatState,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    for _ in 0..waves {
        let own_ids: Vec<UnitId> = own_side
            .board
            .iter()
            .filter(|u| u.id != src_id && u.health > 0)
            .map(|u| u.id)
            .collect();
        for tid in own_ids {
            if let Some(pos) = own_side.board.iter().position(|u| u.id == tid) {
                let (_, took) =
                    apply_damage(&mut own_side.board, pos, amount, src_id, false, events);
                if took {
                    cards::on_damage_taken(&own_side.board[pos], &mut own_side.hand, rng);
                    cards::on_damage_dealt(
                        &mut own_side.ctx(side, next_id, rng, events),
                        src_id,
                        amount,
                    );
                }
            }
        }
        let opp_ids: Vec<UnitId> = opp_side
            .board
            .iter()
            .filter(|u| u.health > 0)
            .map(|u| u.id)
            .collect();
        for tid in opp_ids {
            if let Some(pos) = opp_side.board.iter().position(|u| u.id == tid) {
                let (_, took) =
                    apply_damage(&mut opp_side.board, pos, amount, src_id, false, events);
                if took {
                    cards::on_damage_taken(&opp_side.board[pos], &mut opp_side.hand, rng);
                    cards::on_damage_dealt(
                        &mut own_side.ctx(side, next_id, rng, events),
                        src_id,
                        amount,
                    );
                }
            }
        }
        resolve_effect_deaths(side, own_side, opp_side, next_id, rng, events);
    }
}

/// Resolve `src_id`'s Start-of-Combat immediate attacks ([`SocAction::Attack`]).
#[allow(clippy::too_many_arguments)]
fn resolve_start_of_combat_attacks(
    side: Side,
    src_id: UnitId,
    strikes: u32,
    own_side: &mut SideCombatState,
    opp_side: &mut SideCombatState,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    for _ in 0..strikes {
        if opp_side.is_empty()
            || !own_side
                .board
                .iter()
                .any(|u| u.id == src_id && u.health > 0 && u.attack > 0)
        {
            break;
        }
        perform_one_strike(
            side, src_id, own_side, opp_side, false, next_id, rng, events,
        );
    }
}

fn perform_attack_turn(
    side: Side,
    atk_side: &mut SideCombatState,
    def_side: &mut SideCombatState,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    let Some((attacker_id, has_windfury)) = select_attacker(&atk_side.board, atk_side.ptr) else {
        return;
    };

    perform_one_strike(
        side,
        attacker_id,
        atk_side,
        def_side,
        true,
        next_id,
        rng,
        events,
    );

    if has_windfury
        && !def_side.is_empty()
        && atk_side
            .board
            .iter()
            .any(|u| u.id == attacker_id && u.attack > 0)
    {
        perform_one_strike(
            side,
            attacker_id,
            atk_side,
            def_side,
            true,
            next_id,
            rng,
            events,
        );
    }
}

fn select_attacker(board: &[Unit], ptr: Option<UnitId>) -> Option<(UnitId, bool)> {
    if board.is_empty() {
        return None;
    }
    let start_idx = ptr
        .and_then(|id| board.iter().position(|u| u.id == id))
        .unwrap_or(0);

    for offset in 0..board.len() {
        let idx = (start_idx + offset) % board.len();
        let u = &board[idx];
        if u.attack > 0 {
            return Some((u.id, u.windfury));
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn perform_one_strike(
    side: Side,
    attacker_id: UnitId,
    atk_side: &mut SideCombatState,
    def_side: &mut SideCombatState,
    advance_ptr: bool,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    let Some(initial_atk_pos) = atk_side.board.iter().position(|u| u.id == attacker_id) else {
        return;
    };
    break_stealth_if_all_hidden(&mut def_side.board);
    let Some(initial_def_pos) = choose_target(&def_side.board, rng) else {
        return;
    };
    let target_id = def_side.board[initial_def_pos].id;

    // Attacking breaks Stealth.
    atk_side.board[initial_atk_pos].stealth = false;

    // 1. Emit AttackDeclared when the attack is declared (before Rally and on-attack observers).
    events.push(Event::AttackDeclared {
        side,
        attacker: attacker_id,
        target: target_id,
    });

    // 2. Fire On-Attack (Rally) hook before damage.
    let attacker_card_id = atk_side.board[initial_atk_pos].card_id;
    let before_rally: Vec<(UnitId, i32, i32)> = atk_side
        .board
        .iter()
        .map(|u| (u.id, u.attack, u.health))
        .collect();
    let events_len_before_rally = events.len();
    let mut generated_hand = Vec::new();
    let (rally_summons, summons_attack_target) = cards::on_rally(
        side,
        &mut atk_side.board,
        initial_atk_pos,
        Some((&mut def_side.board, initial_def_pos)),
        &mut def_side.hand,
        &mut atk_side.auras,
        &mut atk_side.hand,
        &mut atk_side.hand_summoned,
        &mut generated_hand,
        &mut Vec::new(), // spells cast by a Rally in the Tavern only
        rng,
        events,
    );
    let already_logged_units: Vec<UnitId> = events[events_len_before_rally..]
        .iter()
        .filter_map(|ev| match *ev {
            Event::StatBuff {
                side: ev_side,
                unit,
                ..
            } if ev_side == side => Some(unit),
            _ => None,
        })
        .collect();
    for (id, old_atk, old_hp) in before_rally {
        if already_logged_units.contains(&id) {
            continue;
        }
        if let Some(u) = atk_side.board.iter().find(|u| u.id == id) {
            if u.attack != old_atk || u.health != old_hp {
                events.push(Event::StatBuff {
                    side,
                    unit: id,
                    atk_delta: u.attack - old_atk,
                    hp_delta: u.health - old_hp,
                    attack: u.attack,
                    health: u.health,
                    reason: "Rally",
                });
            }
        }
    }
    for mut card in generated_hand {
        if atk_side.hand.len() < 10 {
            cards::sync_unit_auras(&mut card, &atk_side.auras);
            atk_side.hand.push(card);
            atk_side.hand_summoned.push(false);
            atk_side.hand_given_by.push(None);
            cards::on_card_added_to_hand(&atk_side.board, &mut atk_side.auras);
        }
    }
    let mut forced_striker: Option<UnitId> = None;
    if !rally_summons.is_empty() {
        let mut insert_pos = initial_atk_pos + 1;
        for mut token in rally_summons {
            if atk_side.board.len() < MAX_BOARD_SIZE {
                token.id = *next_id;
                *next_id += 1;
                if summons_attack_target {
                    forced_striker = Some(token.id);
                }
                atk_side.prepare_summoned_unit(&mut token);
                events.push(Event::UnitSummoned {
                    side,
                    source: attacker_id,
                    unit: token.id,
                    name: token.name.clone(),
                    attack: token.attack,
                    health: token.health,
                    reason: "Rally",
                });
                atk_side.board.insert(insert_pos, token);
                insert_pos += 1;
            }
        }
    }

    if cards::is_rally_minion(attacker_card_id) {
        cards::after_friendly_rally(&mut atk_side.ctx(side, next_id, rng, events));
    }

    // 2b. Fire friendly-attack observers.
    cards::on_friendly_attack(&mut atk_side.ctx(side, next_id, rng, events), attacker_id);

    // 2c. Resolve any deaths caused during Rally (`Obsidian Ravager`, `Deathstrider`).
    resolve_effect_deaths(side, atk_side, def_side, next_id, rng, events);

    // 2d. Rally summons that attack the target first (e.g. `Jailbird Juggernaut`'s Golem).
    if let Some(striker_id) = forced_striker {
        perform_forced_target_strike(
            side, striker_id, target_id, atk_side, def_side, next_id, rng, events,
        );
    }

    let Some(atk_pos) = atk_side
        .board
        .iter()
        .position(|u| u.id == attacker_id && u.health > 0)
    else {
        return;
    };
    let Some(def_pos) = def_side
        .board
        .iter()
        .position(|u| u.id == target_id && u.health > 0)
    else {
        if advance_ptr {
            atk_side.ptr = unit_after(&atk_side.board, atk_pos);
        }
        return;
    };

    let attacker_attack = atk_side.board[atk_pos].attack;
    let attacker_venomous = atk_side.board[atk_pos].venomous;

    let target_attack = def_side.board[def_pos].attack;
    let target_venomous = def_side.board[def_pos].venomous;
    let def_pre_hp = def_side.board[def_pos].health;

    // 4. Simultaneous damage (target first, then attacker). A Venomous attacker destroys every
    //    minion its attack damages (cleave included); its Venomous is used up afterwards.
    let (mut atk_consumed_venom, def_took_damage) = apply_damage(
        &mut def_side.board,
        def_pos,
        attacker_attack,
        attacker_id,
        attacker_venomous,
        events,
    );
    if def_took_damage {
        cards::on_damage_taken(&def_side.board[def_pos], &mut def_side.hand, rng);
        cards::on_damage_dealt(
            &mut atk_side.ctx(side, next_id, rng, events),
            attacker_id,
            attacker_attack,
        );
    }

    let (def_consumed_venom, atk_took_damage) = apply_damage(
        &mut atk_side.board,
        atk_pos,
        target_attack,
        target_id,
        target_venomous,
        events,
    );
    if def_consumed_venom {
        def_side.board[def_pos].venomous = false;
    }
    if atk_took_damage {
        cards::on_damage_taken(&atk_side.board[atk_pos], &mut atk_side.hand, rng);
        cards::on_damage_dealt(
            &mut def_side.ctx(side.other(), next_id, rng, events),
            target_id,
            target_attack,
        );
    }

    // 4a. Cleave damage to adjacent enemies (`Blade Collector`).
    if cards::cleaves_adjacent_enemies(atk_side.board[atk_pos].card_id) {
        let mut adj_positions = Vec::with_capacity(2);
        if def_pos > 0 {
            adj_positions.push(def_pos - 1);
        }
        if def_pos + 1 < def_side.board.len() {
            adj_positions.push(def_pos + 1);
        }
        for n_pos in adj_positions {
            let (n_consumed_venom, n_took_damage) = apply_damage(
                &mut def_side.board,
                n_pos,
                attacker_attack,
                attacker_id,
                attacker_venomous,
                events,
            );
            atk_consumed_venom |= n_consumed_venom;
            if n_took_damage {
                cards::on_damage_taken(&def_side.board[n_pos], &mut def_side.hand, rng);
                cards::on_damage_dealt(
                    &mut atk_side.ctx(side, next_id, rng, events),
                    attacker_id,
                    attacker_attack,
                );
            }
        }
    }
    if atk_consumed_venom {
        atk_side.board[atk_pos].venomous = false;
    }

    // 4b. Excess attack damage to adjacent enemies (e.g., `Wildfire Elemental`).
    if def_took_damage && def_side.board[def_pos].health <= 0 {
        let excess = (attacker_attack - def_pre_hp).max(0);
        let has_cleave = cards::deals_excess_damage_to_neighbors(atk_side.board[atk_pos].card_id);
        let is_golden_cleave = atk_side.board[atk_pos].is_golden;
        if excess > 0 && has_cleave {
            let mut neighbors = Vec::with_capacity(2);
            if def_pos > 0 {
                neighbors.push(def_pos - 1);
            }
            if def_pos + 1 < def_side.board.len() {
                neighbors.push(def_pos + 1);
            }
            let targets: Vec<usize> = if !is_golden_cleave && !neighbors.is_empty() {
                let pick = if neighbors.len() == 1 {
                    0
                } else {
                    rng.below(neighbors.len())
                };
                vec![neighbors[pick]]
            } else {
                neighbors
            };
            for n_pos in targets {
                let (_, n_took_damage) = apply_damage(
                    &mut def_side.board,
                    n_pos,
                    excess,
                    attacker_id,
                    false,
                    events,
                );
                if n_took_damage {
                    cards::on_damage_taken(&def_side.board[n_pos], &mut def_side.hand, rng);
                    cards::on_damage_dealt(
                        &mut atk_side.ctx(side, next_id, rng, events),
                        attacker_id,
                        excess,
                    );
                }
            }
        }
    }

    // 4c. After-attack bonus damage (`De-volition-ist`).
    let after_dmg = cards::after_attack_damage(&atk_side.board[atk_pos]);
    if after_dmg > 0 {
        let max_hp = def_side
            .board
            .iter()
            .filter(|u| u.health > 0)
            .map(|u| u.health)
            .max();
        if let Some(m_hp) = max_hp {
            let candidates: Vec<usize> = def_side
                .board
                .iter()
                .enumerate()
                .filter(|(_, u)| u.health == m_hp)
                .map(|(i, _)| i)
                .collect();
            let pick_pos = if candidates.len() == 1 {
                candidates[0]
            } else {
                candidates[rng.below(candidates.len())]
            };
            let (_, took) = apply_damage(
                &mut def_side.board,
                pick_pos,
                after_dmg,
                attacker_id,
                false,
                events,
            );
            if took {
                cards::on_damage_taken(&def_side.board[pick_pos], &mut def_side.hand, rng);
                cards::on_damage_dealt(
                    &mut atk_side.ctx(side, next_id, rng, events),
                    attacker_id,
                    after_dmg,
                );
            }
        }
    }

    // 5. Emit Deaths (defending side first left->right, then attacking side left->right).
    for u in &def_side.board {
        if u.health <= 0 {
            events.push(Event::Death { unit: u.id });
        }
    }
    for u in &atk_side.board {
        if u.health <= 0 {
            events.push(Event::Death { unit: u.id });
        }
    }

    // 6. Point the attacking side at the unit right of the attacker, or at the attacker if it
    //    died, while dead units are still in place; deaths re-anchor pointers on dead units so
    //    that the units summoned into their slots take their place (docs/combat.md §5.4).
    if advance_ptr {
        atk_side.ptr = if atk_side.board[atk_pos].health > 0 {
            unit_after(&atk_side.board, atk_pos)
        } else {
            Some(attacker_id)
        };
    }

    // 7. Resolve deaths (defending side first, then attacking side).
    resolve_deaths(side.other(), def_side, atk_side, next_id, rng, events);
    resolve_deaths(side, atk_side, def_side, next_id, rng, events);
    resolve_pending_immediate_attacks(side, atk_side, def_side, next_id, rng, events);
}

#[allow(clippy::too_many_arguments)]
fn perform_forced_target_strike(
    side: Side,
    attacker_id: UnitId,
    target_id: UnitId,
    atk_side: &mut SideCombatState,
    def_side: &mut SideCombatState,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    if !atk_side
        .board
        .iter()
        .any(|u| u.id == attacker_id && u.health > 0)
        || !def_side
            .board
            .iter()
            .any(|u| u.id == target_id && u.health > 0)
    {
        return;
    }

    events.push(Event::AttackDeclared {
        side,
        attacker: attacker_id,
        target: target_id,
    });

    cards::on_friendly_attack(&mut atk_side.ctx(side, next_id, rng, events), attacker_id);

    let Some(atk_pos) = atk_side
        .board
        .iter()
        .position(|u| u.id == attacker_id && u.health > 0)
    else {
        return;
    };
    let Some(def_pos) = def_side
        .board
        .iter()
        .position(|u| u.id == target_id && u.health > 0)
    else {
        return;
    };

    let attacker_attack = atk_side.board[atk_pos].attack;
    let attacker_venomous = atk_side.board[atk_pos].venomous;
    let target_attack = def_side.board[def_pos].attack;
    let target_venomous = def_side.board[def_pos].venomous;

    let (atk_consumed_venom, def_took_damage) = apply_damage(
        &mut def_side.board,
        def_pos,
        attacker_attack,
        attacker_id,
        attacker_venomous,
        events,
    );
    if atk_consumed_venom {
        atk_side.board[atk_pos].venomous = false;
    }
    if def_took_damage {
        cards::on_damage_taken(&def_side.board[def_pos], &mut def_side.hand, rng);
        cards::on_damage_dealt(
            &mut atk_side.ctx(side, next_id, rng, events),
            attacker_id,
            attacker_attack,
        );
    }

    let (def_consumed_venom, atk_took_damage) = apply_damage(
        &mut atk_side.board,
        atk_pos,
        target_attack,
        target_id,
        target_venomous,
        events,
    );
    if def_consumed_venom {
        def_side.board[def_pos].venomous = false;
    }
    if atk_took_damage {
        cards::on_damage_taken(&atk_side.board[atk_pos], &mut atk_side.hand, rng);
        cards::on_damage_dealt(
            &mut def_side.ctx(side.other(), next_id, rng, events),
            target_id,
            target_attack,
        );
    }

    for u in &def_side.board {
        if u.health <= 0 {
            events.push(Event::Death { unit: u.id });
        }
    }
    for u in &atk_side.board {
        if u.health <= 0 {
            events.push(Event::Death { unit: u.id });
        }
    }

    resolve_deaths(side.other(), def_side, atk_side, next_id, rng, events);
    resolve_deaths(side, atk_side, def_side, next_id, rng, events);
    resolve_pending_immediate_attacks(side, atk_side, def_side, next_id, rng, events);
}

fn resolve_pending_immediate_attacks(
    side_a_turn: Side,
    side_a: &mut SideCombatState,
    side_b: &mut SideCombatState,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    loop {
        if let Some(attacker_id) = side_a.pending_immediate_attacks.first().copied() {
            side_a.pending_immediate_attacks.remove(0);
            if !side_b.is_empty()
                && side_a
                    .board
                    .iter()
                    .any(|u| u.id == attacker_id && u.health > 0 && u.attack > 0)
            {
                perform_one_strike(
                    side_a_turn,
                    attacker_id,
                    side_a,
                    side_b,
                    false,
                    next_id,
                    rng,
                    events,
                );
            }
            continue;
        }
        if let Some(attacker_id) = side_b.pending_immediate_attacks.first().copied() {
            side_b.pending_immediate_attacks.remove(0);
            if !side_a.is_empty()
                && side_b
                    .board
                    .iter()
                    .any(|u| u.id == attacker_id && u.health > 0 && u.attack > 0)
            {
                perform_one_strike(
                    side_a_turn.other(),
                    attacker_id,
                    side_b,
                    side_a,
                    false,
                    next_id,
                    rng,
                    events,
                );
            }
            continue;
        }
        break;
    }
}

/// If every defender has Stealth, they all lose it, so that they can be attacked
/// (`docs/combat.md` §5.3.1 step 2).
fn break_stealth_if_all_hidden(defenders: &mut [Unit]) {
    if defenders.iter().all(|u| u.stealth) {
        for u in defenders.iter_mut() {
            u.stealth = false;
        }
    }
}

fn choose_target(defenders: &[Unit], rng: &mut Rng) -> Option<usize> {
    // Stealthed minions cannot be attacked (an all-Stealth side has already lost it).
    let visible: Vec<usize> = defenders
        .iter()
        .enumerate()
        .filter(|(_, u)| !u.stealth)
        .map(|(i, _)| i)
        .collect();

    let taunts: Vec<usize> = visible
        .iter()
        .copied()
        .filter(|&i| defenders[i].taunt)
        .collect();

    let candidates = if !taunts.is_empty() { taunts } else { visible };

    match candidates.len() {
        0 => None,
        1 => Some(candidates[0]),
        n => Some(candidates[rng.below(n)]),
    }
}

/// Apply `amount` damage from `source_id` to `board[pos]`.
/// Returns `(venomous_consumed, health_damage_dealt)`.
fn apply_damage(
    board: &mut [Unit],
    pos: usize,
    amount: i32,
    source_id: UnitId,
    source_venomous: bool,
    events: &mut Vec<Event>,
) -> (bool, bool) {
    if amount <= 0 {
        return (false, false);
    }
    if board[pos].divine_shield {
        cards::pop_divine_shield(board, pos, events);
        (false, false)
    } else {
        let unit = &mut board[pos];
        unit.health -= amount;
        events.push(Event::DamageDealt {
            unit: unit.id,
            amount,
            from: source_id,
        });
        let res = if source_venomous {
            if unit.health > 0 {
                unit.health = 0;
            }
            events.push(Event::VenomousTriggered {
                attacker: source_id,
                target: unit.id,
            });
            (true, true)
        } else {
            (false, true)
        };
        if unit.health <= 0 {
            unit.killed_by = Some(source_id);
        }
        res
    }
}

/// The unit immediately to the right of `board[idx]`, wrapping to the leftmost.
fn unit_after(board: &[Unit], idx: usize) -> Option<UnitId> {
    if board.is_empty() {
        return None;
    }
    Some(board[(idx + 1) % board.len()].id)
}

/// Re-anchor a side's next-attacker pointer after a round of its deaths resolved
/// (`docs/combat.md` §5.4). A pointer on a unit that died moves to the left-most unit summoned
/// into that unit's slot (`slot_summons`: the ids each dead unit's Deathrattle and Reborn
/// summoned), otherwise on to the next unit of `old_ids` (the board before the deaths), and
/// past the rightmost wraps to the leftmost unit.
fn reanchor_ptr(
    ptr: Option<UnitId>,
    old_ids: &[UnitId],
    slot_summons: &[(UnitId, Range<UnitId>)],
    board: &[Unit],
) -> Option<UnitId> {
    let leftmost = board.first()?.id;
    let on_board = |id: UnitId| board.iter().any(|u| u.id == id);
    let Some(start) = ptr.and_then(|p| old_ids.iter().position(|&id| id == p)) else {
        return Some(ptr.filter(|&p| on_board(p)).unwrap_or(leftmost));
    };
    for &id in &old_ids[start..] {
        if on_board(id) {
            return Some(id);
        }
        let summoned = board.iter().find(|u| {
            slot_summons
                .iter()
                .any(|(dead, ids)| *dead == id && ids.contains(&u.id))
        });
        if let Some(u) = summoned {
            return Some(u.id);
        }
    }
    Some(leftmost)
}

fn resolve_deaths(
    side: Side,
    side_state: &mut SideCombatState,
    opp_side: &mut SideCombatState,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    if !side_state.board.iter().any(|u| u.health <= 0) {
        return;
    }

    // Living units always retain their board slots; summons insert at `cursor`
    // while `side_state.board.len() < MAX_BOARD_SIZE`.
    let old_board = side_state.board.clone();
    side_state.board.retain(|u| u.health > 0);
    let mut cursor: usize = 0;
    let mut killed_opp = false;
    // Living friendly units destroyed by a Deathrattle stay in place until a further round.
    let mut killed_own = false;
    // Dead units with Reborn and their slots (after their Deathrattle summons), resummoned once
    // all of this side's deaths have resolved (docs/combat.md §5.3.1 step 9).
    let mut reborns: Vec<(Unit, usize)> = Vec::new();
    // The board before these deaths, and the ids of the units each dead unit's Deathrattle and
    // Reborn summoned into its slot, to re-anchor the pointer (docs/combat.md §5.4).
    let old_ids: Vec<UnitId> = old_board.iter().map(|u| u.id).collect();
    let mut slot_summons: Vec<(UnitId, Range<UnitId>)> = Vec::new();

    for unit in old_board {
        if unit.health > 0 {
            cursor += 1;
            continue;
        }

        // Unit died.
        side_state.dead_units.push(unit.clone());
        cards::on_friendly_death(&mut side_state.ctx(side, next_id, rng, events), &unit);

        if unit.tribe.matches(Tribe::Aberration) {
            if !unit.is_deity {
                side_state.dead_aberrations.push(unit.clone());
            }
            if !side_state.deity_awakened {
                side_state.aberration_deaths += 1;
            }
        }

        // 1. Unified Deathrattle (summons at `cursor` and/or board buffs)
        let dr_repeats = 1 + cards::board_passive(&side_state.board, Passive::ExtraDeathrattles);
        let mut destroys = Vec::new();
        let first_summon = *next_id;
        for _ in 0..dr_repeats {
            let mut dr_ctx = side_state.ctx(side, next_id, rng, events);
            dr_ctx.cursor = cursor;
            cards::on_deathrattle(&unit, &mut dr_ctx);
            cursor = dr_ctx.cursor;
            destroys.append(&mut dr_ctx.destroys);
        }
        slot_summons.push((unit.id, first_summon..*next_id));
        for target_id in destroys {
            let is_target = |u: &&mut Unit| u.id == target_id && u.health > 0;
            if let Some(target) = opp_side.board.iter_mut().find(is_target) {
                target.health = 0;
                events.push(Event::Death { unit: target.id });
                killed_opp = true;
            } else if let Some(target) = side_state.board.iter_mut().find(is_target) {
                target.health = 0;
                events.push(Event::Death { unit: target.id });
                killed_own = true;
            }
        }

        if unit.reborn {
            reborns.push((unit, cursor));
        }
    }

    // 2. Deity Awakening, after all of this side's deaths and before any Reborn.
    awaken_deity_if_due(side, side_state, next_id, rng, events);

    // 3. Reborn resummons, left to right; each slot shifts by the units inserted before it.
    let mut inserted = 0;
    for (unit, slot) in reborns {
        let first_summon = *next_id;
        let mut ctx = side_state.ctx(side, next_id, rng, events);
        ctx.cursor = slot + inserted;
        cards::reborn(&mut ctx, &unit);
        inserted = ctx.cursor - slot;
        slot_summons.push((unit.id, first_summon..*next_id));
    }

    // 4. Effects that fill open board slots after deaths (e.g. `Boon of Beetles`).
    cards::after_combat_deaths(&mut side_state.ctx(side, next_id, rng, events));

    // 5. Synchronize dynamic friendly-death and persistent auras.
    let before_aura = cards::stat_snapshot(&side_state.board);
    cards::sync_combat_auras(
        &mut side_state.board,
        &mut side_state.hand,
        &side_state.auras,
    );
    cards::push_stat_changes(side, &side_state.board, &before_aura, "Death Aura", events);

    // 6. Re-anchor the pointer if its unit died.
    side_state.ptr = reanchor_ptr(side_state.ptr, &old_ids, &slot_summons, &side_state.board);

    if killed_own {
        resolve_deaths(side, side_state, opp_side, next_id, rng, events);
    }
    if killed_opp {
        resolve_deaths(side.other(), opp_side, side_state, next_id, rng, events);
    }
}

/// Awaken the side's Deity at the right end of its board once 4 friendly Aberrations have died
/// this combat (Patch 36.6.3) and there is room; called after a round of deaths resolves, before
/// its Reborns (`docs/combat.md` §5.3.1 step 9).
fn awaken_deity_if_due(
    side: Side,
    side_state: &mut SideCombatState,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    if side_state.deity_awakened
        || side_state.auras.deity.kind == DeityKind::None
        || side_state.aberration_deaths < cards::DEITY_SACRIFICE_REQUIREMENT
        || side_state.board.len() >= MAX_BOARD_SIZE
    {
        return;
    }
    side_state.deity_awakened = true;
    let deity_kind = side_state.auras.deity.kind;
    let mut deity_unit = cards::instantiate_deity(&side_state.auras.deity);
    deity_unit.id = *next_id;
    *next_id += 1;
    cards::apply_combat_summon_modifiers(
        &mut side_state.board,
        &side_state.auras,
        side_state.combat_beast_bonus_atk,
        &mut deity_unit,
    );
    let deity_id = deity_unit.id;
    let deity_name = deity_unit.name.clone();
    let deity_idx = side_state.board.len();
    side_state.board.push(deity_unit);

    events.push(Event::DeityAwakened {
        side,
        deity: deity_kind,
        unit: deity_id,
        name: deity_name,
    });

    cards::on_awaken(&mut side_state.ctx(side, next_id, rng, events), deity_idx);
}
