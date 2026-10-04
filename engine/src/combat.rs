//! Card-agnostic combat resolution loop (`docs/combat.md`).

use crate::cards::{self, BoardCtx, Passive};
use crate::cards::deities::{self, DEITY_SACRIFICE_REQUIREMENT};
use crate::events::Event;
use crate::model::{BattleOutcome, GameState, PlayerAuras, Side, Tribe, Unit, UnitId};
use crate::rng::Rng;

/// Maximum number of units a board can hold at once.
pub const MAX_BOARD_SIZE: usize = 7;

/// Complete result of a single deterministic battle (`docs/combat.md` §5.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BattleResult {
    pub outcome: BattleOutcome,
    pub survivors_a: Vec<Unit>,
    pub survivors_b: Vec<Unit>,
    pub hero_damage: u32,
    pub events: Vec<Event>,
    pub hand_a: Vec<Unit>,
    pub hand_b: Vec<Unit>,
    pub eternal_knights_died_a: u32,
    pub eternal_knights_died_b: u32,
    pub auras_a: PlayerAuras,
    pub auras_b: PlayerAuras,
    pub dead_units_a: Vec<Unit>,
    pub dead_units_b: Vec<Unit>,
}

/// Per-side combat state tracked during a single battle.
struct SideCombatState {
    board: Vec<Unit>,
    ptr: Option<UnitId>,
    hero_tier: u32,
    auras: PlayerAuras,
    hand: Vec<Unit>,
    hand_summoned: Vec<bool>,
    combat_beast_bonus_atk: i32,
    friendly_deaths_this_combat: u32,
    aberration_deaths: u32,
    deity_awakened: bool,
    dead_aberrations: Vec<Unit>,
    dead_units: Vec<Unit>,
    pending_immediate_attacks: Vec<UnitId>,
}

impl SideCombatState {
    fn new(
        mut board: Vec<Unit>,
        hero_tier: u32,
        auras: PlayerAuras,
        mut hand: Vec<Unit>,
    ) -> Self {
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
        let ptr = board.first().map(|u| u.id);
        Self {
            board,
            ptr,
            hero_tier,
            auras,
            hand,
            hand_summoned,
            combat_beast_bonus_atk: 0,
            friendly_deaths_this_combat: 0,
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
            cursor: 0,
            auras: &mut self.auras,
            hand: &mut self.hand,
            hand_summoned: &mut self.hand_summoned,
            dead_aberrations: &self.dead_aberrations,
            beast_bonus_atk: &mut self.combat_beast_bonus_atk,
            hero_tier: self.hero_tier,
            pending_attacks: &mut self.pending_immediate_attacks,
            enemy_destroys: Vec::new(),
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
}

/// Resolve one deterministic battle from `(board_a, board_b, state, seed)`.
pub fn resolve_battle(
    board_a: &[Unit],
    board_b: &[Unit],
    state: &GameState,
    seed: u64,
) -> BattleResult {
    let mut next_id: UnitId = 0;

    let mut units_a: Vec<Unit> = board_a
        .iter()
        .cloned()
        .map(|mut u| {
            u.id = next_id;
            next_id += 1;
            u
        })
        .collect();
    let mut units_b: Vec<Unit> = board_b
        .iter()
        .cloned()
        .map(|mut u| {
            u.id = next_id;
            next_id += 1;
            u
        })
        .collect();

    units_a.truncate(MAX_BOARD_SIZE);
    units_b.truncate(MAX_BOARD_SIZE);

    let mut rng = Rng::new(seed);

    let first_attacker = if units_a.len() > units_b.len() {
        Side::A
    } else if units_b.len() > units_a.len() {
        Side::B
    } else if rng.below(2) == 0 {
        Side::A
    } else {
        Side::B
    };

    let mut events = vec![Event::BattleStart {
        seed,
        first_attacker,
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

    // Start of Combat triggers: Side A first, then Side B, left-to-right.
    resolve_start_of_combat_spells(Side::A, &mut side_a, &mut side_b, &mut rng, &mut events);
    cards::on_start_of_combat(&mut side_a.ctx(Side::A, &mut next_id, &mut rng, &mut events));
    resolve_start_of_combat_destroys(
        Side::A,
        &mut side_a,
        &mut side_b,
        &mut next_id,
        &mut rng,
        &mut events,
    );
    resolve_start_of_combat_spells(Side::B, &mut side_b, &mut side_a, &mut rng, &mut events);
    cards::on_start_of_combat(&mut side_b.ctx(Side::B, &mut next_id, &mut rng, &mut events));
    resolve_start_of_combat_destroys(
        Side::B,
        &mut side_b,
        &mut side_a,
        &mut next_id,
        &mut rng,
        &mut events,
    );

    resolve_start_of_combat_aoe(
        Side::A,
        &mut side_a,
        &mut side_b,
        &mut next_id,
        &mut rng,
        &mut events,
    );
    resolve_start_of_combat_aoe(
        Side::B,
        &mut side_b,
        &mut side_a,
        &mut next_id,
        &mut rng,
        &mut events,
    );

    resolve_start_of_combat_attacks(
        Side::A,
        &mut side_a,
        &mut side_b,
        &mut next_id,
        &mut rng,
        &mut events,
    );
    resolve_start_of_combat_attacks(
        Side::B,
        &mut side_b,
        &mut side_a,
        &mut next_id,
        &mut rng,
        &mut events,
    );

    let mut current = first_attacker;
    let mut stalled_draw = false;

    while !side_a.is_empty() && !side_b.is_empty() {
        let (atk_side, def_side) = match current {
            Side::A => (&mut side_a, &mut side_b),
            Side::B => (&mut side_b, &mut side_a),
        };

        if atk_side.can_attack() {
            perform_attack_turn(
                current,
                atk_side,
                def_side,
                &mut next_id,
                &mut rng,
                &mut events,
            );
            current = current.other();
        } else if !def_side.can_attack() {
            stalled_draw = true;
            break;
        } else {
            events.push(Event::TurnSkipped { side: current });
            current = current.other();
        }
    }

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
        eternal_knights_died_a: side_a.auras.eternal_knights_died,
        eternal_knights_died_b: side_b.auras.eternal_knights_died,
        auras_a: side_a.auras,
        auras_b: side_b.auras,
        dead_units_a: side_a.dead_units,
        dead_units_b: side_b.dead_units,
    }
}

/// Resolve Start-of-Combat Tavern spells (`Brood of Nozdormu`, `Upper Hand`, `Sharing is Caring`).
fn resolve_start_of_combat_spells(
    side: Side,
    own_side: &mut SideCombatState,
    opp_side: &mut SideCombatState,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    while own_side.auras.brood_of_nozdormu_stacks > 0 {
        own_side.auras.brood_of_nozdormu_stacks -= 1;
        if let Some(leftmost) = own_side.board.first_mut() {
            let add = leftmost.attack;
            leftmost.add_stats(add, 0);
            events.push(Event::StatBuff {
                side,
                unit: leftmost.id,
                atk_delta: add,
                hp_delta: 0,
                attack: leftmost.attack,
                health: leftmost.health,
                reason: "Brood of Nozdormu",
            });
        }
    }
    while own_side.auras.upper_hand_stacks > 0 {
        own_side.auras.upper_hand_stacks -= 1;
        let candidates: Vec<usize> = opp_side
            .board
            .iter()
            .enumerate()
            .filter(|(_, u)| u.health > 0)
            .map(|(i, _)| i)
            .collect();
        if !candidates.is_empty() {
            let idx = if candidates.len() == 1 {
                candidates[0]
            } else {
                candidates[rng.below(candidates.len())]
            };
            opp_side.board[idx].health = 1;
            opp_side.board[idx].max_health = opp_side.board[idx].max_health.max(1);
        }
    }
    while own_side.auras.sharing_is_caring_stacks > 0 {
        own_side.auras.sharing_is_caring_stacks -= 1;
        if let (Some(leftmost), Some(nearest_opp)) = (
            own_side.board.first_mut(),
            opp_side.board.iter().find(|u| u.health > 0),
        ) {
            let add_atk = nearest_opp.attack.max(0);
            let add_hp = nearest_opp.health.max(0);
            leftmost.add_stats(add_atk, add_hp);
            events.push(Event::StatBuff {
                side,
                unit: leftmost.id,
                atk_delta: add_atk,
                hp_delta: add_hp,
                attack: leftmost.attack,
                health: leftmost.health,
                reason: "Sharing is Caring",
            });
        }
    }
}

/// Resolve Start-of-Combat friendly neighbor destroys (`Stitched Salvager`).
fn resolve_start_of_combat_destroys(
    side: Side,
    own_side: &mut SideCombatState,
    opp_side: &mut SideCombatState,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    let sources: Vec<(UnitId, bool)> = own_side
        .board
        .iter()
        .filter(|u| u.card_id == cards::tier7::stitched_salvager::ID && u.health > 0)
        .map(|u| (u.id, u.is_golden))
        .collect();
    for (src_id, is_golden) in sources {
        let Some(src_pos) = own_side
            .board
            .iter()
            .position(|u| u.id == src_id && u.health > 0)
        else {
            continue;
        };
        let mut victim_positions = Vec::new();
        if src_pos > 0
            && own_side.board[src_pos - 1].health > 0
            && own_side.board[src_pos - 1].card_id != cards::tier7::stitched_salvager::ID
        {
            victim_positions.push(src_pos - 1);
        }
        if is_golden
            && src_pos + 1 < own_side.board.len()
            && own_side.board[src_pos + 1].health > 0
            && own_side.board[src_pos + 1].card_id != cards::tier7::stitched_salvager::ID
        {
            victim_positions.push(src_pos + 1);
        }
        if victim_positions.is_empty() {
            continue;
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
        own_side.board[src_pos].stitched_stored.extend(stored_copies);
        own_side.ptr = preserve_defender_ptr(&own_side.board, own_side.ptr);
        resolve_deaths(side, own_side, opp_side, next_id, rng, events);
        resolve_pending_immediate_attacks(side, own_side, opp_side, next_id, rng, events);
    }
}

/// Resolve Start-of-Combat board-wide AoE damage (`Boom-in-a-Box`).
fn resolve_start_of_combat_aoe(
    side: Side,
    own_side: &mut SideCombatState,
    opp_side: &mut SideCombatState,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    let sources: Vec<(UnitId, bool)> = own_side
        .board
        .iter()
        .filter(|u| u.card_id == cards::tier4::boom_in_a_box::ID && u.health > 0)
        .map(|u| (u.id, u.is_golden))
        .collect();
    for (src_id, is_golden) in sources {
        if !own_side.board.iter().any(|u| u.id == src_id && u.health > 0) {
            continue;
        }
        let waves = if is_golden { 2 } else { 1 };
        for _ in 0..waves {
            let own_ids: Vec<UnitId> = own_side
                .board
                .iter()
                .filter(|u| u.id != src_id && u.health > 0)
                .map(|u| u.id)
                .collect();
            for tid in own_ids {
                if let Some(pos) = own_side.board.iter().position(|u| u.id == tid) {
                    let (_, took) = apply_damage(&mut own_side.board, pos, 3, src_id, false, events);
                    if took {
                        cards::on_damage_taken(&own_side.board[pos], &mut own_side.hand, rng);
                        cards::on_damage_dealt(
                            &mut own_side.ctx(side, next_id, rng, events),
                            src_id,
                            3,
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
                    let (_, took) = apply_damage(&mut opp_side.board, pos, 3, src_id, false, events);
                    if took {
                        cards::on_damage_taken(&opp_side.board[pos], &mut opp_side.hand, rng);
                        cards::on_damage_dealt(
                            &mut own_side.ctx(side, next_id, rng, events),
                            src_id,
                            3,
                        );
                    }
                }
            }
            for u in &opp_side.board {
                if u.health <= 0 {
                    events.push(Event::Death { unit: u.id });
                }
            }
            for u in &own_side.board {
                if u.health <= 0 {
                    events.push(Event::Death { unit: u.id });
                }
            }
            resolve_deaths(side.other(), opp_side, own_side, next_id, rng, events);
            resolve_deaths(side, own_side, opp_side, next_id, rng, events);
            resolve_pending_immediate_attacks(side, own_side, opp_side, next_id, rng, events);
        }
    }
}

/// Resolve Start-of-Combat immediate attacks (`Heroic Broodmother`).
fn resolve_start_of_combat_attacks(
    side: Side,
    own_side: &mut SideCombatState,
    opp_side: &mut SideCombatState,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    let sources: Vec<(UnitId, bool)> = own_side
        .board
        .iter()
        .filter(|u| u.card_id == cards::tier6::heroic_broodmother::ID && u.health > 0)
        .map(|u| (u.id, u.is_golden))
        .collect();
    for (src_id, is_golden) in sources {
        let strikes = if is_golden { 2 } else { 1 };
        for _ in 0..strikes {
            if opp_side.is_empty()
                || !own_side
                    .board
                    .iter()
                    .any(|u| u.id == src_id && u.health > 0 && u.attack > 0)
            {
                break;
            }
            perform_one_strike(side, src_id, own_side, opp_side, false, next_id, rng, events);
        }
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
        && atk_side.board.iter().any(|u| u.id == attacker_id && u.attack > 0)
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
        &mut atk_side.auras,
        &atk_side.hand,
        &mut atk_side.hand_summoned,
        &mut generated_hand,
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
        let prev_hand_len = atk_side.hand.len();
        cards::after_friendly_rally(&mut atk_side.ctx(side, next_id, rng, events));
        for _ in prev_hand_len..atk_side.hand.len() {
            cards::on_card_added_to_hand(&atk_side.board, &mut atk_side.auras);
        }
    }

    // 2b. Fire friendly-attack observers.
    cards::on_friendly_attack(&mut atk_side.ctx(side, next_id, rng, events), attacker_id);

    // 2c. Resolve any deaths caused during Rally (`Obsidian Ravager`, `Deathstrider`).
    if def_side.board.iter().any(|u| u.health <= 0)
        || atk_side.board.iter().any(|u| u.health <= 0)
    {
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
        atk_side.ptr = preserve_defender_ptr(&atk_side.board, atk_side.ptr);
        def_side.ptr = preserve_defender_ptr(&def_side.board, def_side.ptr);
        resolve_deaths(side.other(), def_side, atk_side, next_id, rng, events);
        resolve_deaths(side, atk_side, def_side, next_id, rng, events);
        resolve_pending_immediate_attacks(side, atk_side, def_side, next_id, rng, events);
    }

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
            atk_side.ptr = advance_attacker_ptr(&atk_side.board, atk_pos);
        }
        return;
    };

    let attacker_attack = atk_side.board[atk_pos].attack;
    let attacker_venomous = atk_side.board[atk_pos].venomous;

    let target_attack = def_side.board[def_pos].attack;
    let target_venomous = def_side.board[def_pos].venomous;
    let def_pre_hp = def_side.board[def_pos].health;

    // 4. Simultaneous damage (target first, then attacker).
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
            let (_, n_took_damage) = apply_damage(
                &mut def_side.board,
                n_pos,
                attacker_attack,
                attacker_id,
                false,
                events,
            );
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

    // 6. Re-anchor both pointers while dead units are still in place.
    if advance_ptr {
        atk_side.ptr = advance_attacker_ptr(&atk_side.board, atk_pos);
    } else {
        atk_side.ptr = preserve_defender_ptr(&atk_side.board, atk_side.ptr);
    }
    def_side.ptr = preserve_defender_ptr(&def_side.board, def_side.ptr);

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

    atk_side.ptr = preserve_defender_ptr(&atk_side.board, atk_side.ptr);
    def_side.ptr = preserve_defender_ptr(&def_side.board, def_side.ptr);

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

fn choose_target(defenders: &[Unit], rng: &mut Rng) -> Option<usize> {
    if defenders.is_empty() {
        return None;
    }

    // Stealthed minions cannot be targeted unless all defenders are Stealthed.
    let visible: Vec<usize> = defenders
        .iter()
        .enumerate()
        .filter(|(_, u)| !u.stealth)
        .map(|(i, _)| i)
        .collect();

    let pool: Vec<usize> = if visible.is_empty() {
        (0..defenders.len()).collect()
    } else {
        visible
    };

    let taunts: Vec<usize> = pool
        .iter()
        .copied()
        .filter(|&i| defenders[i].taunt)
        .collect();

    let candidates = if !taunts.is_empty() { taunts } else { pool };

    if candidates.len() == 1 {
        Some(candidates[0])
    } else {
        let pick = rng.below(candidates.len());
        Some(candidates[pick])
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

fn advance_attacker_ptr(board: &[Unit], attacker_idx: usize) -> Option<UnitId> {
    let n = board.len();
    if n == 0 {
        return None;
    }
    for offset in 1..=n {
        let idx = (attacker_idx + offset) % n;
        if board[idx].health > 0 {
            return Some(board[idx].id);
        }
    }
    None
}

fn preserve_defender_ptr(board: &[Unit], current_ptr: Option<UnitId>) -> Option<UnitId> {
    let n = board.len();
    if n == 0 {
        return None;
    }
    let start_idx = current_ptr
        .and_then(|id| board.iter().position(|u| u.id == id))
        .unwrap_or(0);

    for offset in 0..n {
        let idx = (start_idx + offset) % n;
        if board[idx].health > 0 {
            return Some(board[idx].id);
        }
    }
    None
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

    for unit in old_board {
        if unit.health > 0 {
            cursor += 1;
            continue;
        }

        // Unit died.
        side_state.friendly_deaths_this_combat += 1;
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
        let mut enemy_destroys = Vec::new();
        for _ in 0..dr_repeats {
            let mut dr_ctx = side_state.ctx(side, next_id, rng, events);
            dr_ctx.cursor = cursor;
            cards::on_deathrattle(&unit, &mut dr_ctx);
            cursor = dr_ctx.cursor;
            enemy_destroys.append(&mut dr_ctx.enemy_destroys);
        }
        for target_id in enemy_destroys {
            if let Some(target) = opp_side
                .board
                .iter_mut()
                .find(|u| u.id == target_id && u.health > 0)
            {
                target.health = 0;
                events.push(Event::Death { unit: target.id });
                killed_opp = true;
            }
        }

        // 2. Reborn resummon
        if unit.reborn {
            let mut ctx = side_state.ctx(side, next_id, rng, events);
            ctx.cursor = cursor;
            cards::reborn(&mut ctx, &unit);
            cursor = ctx.cursor;
        }
    }

    // 3. Check Deity Awakening (4 friendly Aberration deaths in combat, Patch 36.6.3).
    if !side_state.deity_awakened
        && side_state.auras.deity.kind != crate::model::DeityKind::None
        && side_state.aberration_deaths >= DEITY_SACRIFICE_REQUIREMENT
        && side_state.board.len() < MAX_BOARD_SIZE
    {
        side_state.deity_awakened = true;
        let deity_kind = side_state.auras.deity.kind;
        let mut deity_unit = deities::instantiate_deity(&side_state.auras.deity);
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
        let is_cthun = deity_unit.card_id == deities::CARD_CTHUN;
        let deity_idx = side_state.board.len();
        side_state.board.push(deity_unit);

        events.push(Event::DeityAwakened {
            side,
            deity: deity_kind,
            unit: deity_id,
            name: deity_name,
        });

        if is_cthun {
            let before: Vec<(UnitId, i32, i32)> =
                side_state.board.iter().map(|u| (u.id, u.attack, u.health)).collect();
            deities::on_cthun_awaken(&mut side_state.board, deity_idx, rng);
            for (u, (id, old_atk, old_hp)) in side_state.board.iter().zip(before) {
                if u.attack != old_atk || u.health != old_hp {
                    events.push(Event::StatBuff {
                        side,
                        unit: id,
                        atk_delta: u.attack - old_atk,
                        hp_delta: u.health - old_hp,
                        attack: u.attack,
                        health: u.health,
                        reason: "C'Thun Awakening",
                    });
                }
            }
        }
    }

    // 3b. Summon `Boon of Beetles` into any open board slots.
    cards::summon_boon_of_beetles(
        side,
        &mut side_state.board,
        &mut side_state.auras,
        side_state.combat_beast_bonus_atk,
        next_id,
        events,
    );

    // 4. Synchronize dynamic friendly-death and persistent auras.
    let before_aura: Vec<(UnitId, i32, i32)> =
        side_state.board.iter().map(|u| (u.id, u.attack, u.health)).collect();
    cards::sync_combat_auras(
        &mut side_state.board,
        &mut side_state.hand,
        &side_state.auras,
        side_state.friendly_deaths_this_combat,
    );
    for (u, (id, old_atk, old_hp)) in side_state.board.iter().zip(before_aura) {
        if u.attack != old_atk || u.health != old_hp {
            events.push(Event::StatBuff {
                side,
                unit: id,
                atk_delta: u.attack - old_atk,
                hp_delta: u.health - old_hp,
                attack: u.attack,
                health: u.health,
                reason: "Death Aura",
            });
        }
    }

    if side_state.ptr.is_none() && !side_state.board.is_empty() {
        side_state.ptr = Some(side_state.board[0].id);
    }

    if killed_opp {
        opp_side.ptr = preserve_defender_ptr(&opp_side.board, opp_side.ptr);
        resolve_deaths(side.other(), opp_side, side_state, next_id, rng, events);
    }
}
