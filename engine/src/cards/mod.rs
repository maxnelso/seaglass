//! Card templates, catalogs, and unified tier-agnostic per-card hook dispatch.
//!
//! Card definitions are organized by folder (`src/cards/tier1/`, `src/cards/tier2/`,
//! `src/cards/deities.rs`, `src/cards/spells.rs`, `src/cards/tokens.rs`), while all
//! Tavern and Combat hooks are dispatched uniformly by `CardId` in this module without
//! any tier-specific branching.

pub mod deities;
pub mod spells;
pub mod tier1;
pub mod tier2;
pub mod tier3;
pub mod tokens;

use crate::combat::MAX_BOARD_SIZE;
use crate::events::Event;
use crate::model::{CardId, Keyword, PlayerAuras, Side, Tribe, Unit, UnitId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Target domain required by a minion's `Activate` ability (`docs/tavern.md` §5.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActivateTargetKind {
    /// Untargeted (`Clever Castaway`, `Decoy Conjurer`, `Fruit Vendor`, `Hired Mount`).
    None,
    /// Requires another friendly minion on `board` (`Suspicious Prisonguard`).
    BoardOther,
    /// Requires a card in `hand` (`Brain Rotter`, `Abyssal Envoy`, `Mangled Bandit`).
    HandCard,
    /// Requires a card in `shop` (`Lurking Lionfish`).
    ShopCard,
}

/// Unified execution context passed to every `Deathrattle` handler.
///
/// Supports summoning tokens at the dying minion's board slot (`cursor`),
/// buffing friendly minions on the board, mutating persistent `auras`, and
/// adding/modifying cards in `hand` in a single general hook.
pub struct DeathrattleContext<'a> {
    pub side: Side,
    pub board: &'a mut Vec<Unit>,
    pub cursor: &'a mut usize,
    pub auras: &'a mut PlayerAuras,
    pub hand: &'a mut Vec<Unit>,
    pub hand_summoned: &'a mut Vec<bool>,
    pub dead_aberrations: &'a [Unit],
    pub combat_beast_bonus_atk: i32,
    pub next_id: &'a mut UnitId,
    pub rng: &'a mut Rng,
    pub events: &'a mut Vec<Event>,
}

impl<'a> DeathrattleContext<'a> {
    /// Summon `token` at the dying minion's current board position (`*self.cursor`)
    /// if the board has space (`< MAX_BOARD_SIZE`), applying all active combat auras.
    pub fn summon(&mut self, source_id: UnitId, mut token: Unit) {
        if self.board.len() >= MAX_BOARD_SIZE {
            return;
        }
        token.id = *self.next_id;
        *self.next_id += 1;
        sync_unit_auras(&mut token, self.auras);
        if token.tribe.matches(Tribe::Beast) && self.combat_beast_bonus_atk != 0 {
            token.add_stats(self.combat_beast_bonus_atk, 0);
        }
        token.sync_max_stats();
        check_stat_thresholds(&mut token);
        self.events.push(Event::UnitSummoned {
            side: self.side,
            source: source_id,
            unit: token.id,
            name: token.name.clone(),
            attack: token.attack,
            health: token.health,
            reason: "Deathrattle",
        });
        self.board.insert(*self.cursor, token);
        *self.cursor += 1;
    }

    /// Buff `self.board[board_idx]` by `(atk_delta, hp_delta)` and emit a `StatBuff` event.
    pub fn buff_unit(
        &mut self,
        board_idx: usize,
        atk_delta: i32,
        hp_delta: i32,
        reason: &'static str,
    ) {
        if board_idx >= self.board.len() {
            return;
        }
        let u = &mut self.board[board_idx];
        u.add_stats(atk_delta, hp_delta);
        self.events.push(Event::StatBuff {
            side: self.side,
            unit: u.id,
            atk_delta,
            hp_delta,
            attack: u.attack,
            health: u.health,
            reason,
        });
    }

    /// Buff all friendly minions currently on `self.board` by `(atk_delta, hp_delta)`.
    pub fn buff_all_friendly(&mut self, atk_delta: i32, hp_delta: i32, reason: &'static str) {
        for idx in 0..self.board.len() {
            self.buff_unit(idx, atk_delta, hp_delta, reason);
        }
    }

    /// Add `card` to the player's `hand` if `hand.len() < 10`.
    pub fn add_to_hand(&mut self, card: Unit) {
        if self.hand.len() < 10 {
            self.hand.push(card);
            self.hand_summoned.push(false);
        }
    }
}

/// Static definition of a purchasable minion in the shared card pool.
#[derive(Clone, Debug)]
pub struct CardTemplate {
    pub card_id: CardId,
    pub name: String,
    pub attack: i32,
    pub health: i32,
    pub tavern_tier: u32,
    pub tribe: Tribe,
    pub taunt: bool,
    pub divine_shield: bool,
    pub windfury: bool,
    pub reborn: bool,
    pub venomous: bool,
    pub stealth: bool,
    pub magnetic: bool,
    pub intrinsic_golden: bool,
    pub activate_cost: Option<u32>,
}

impl CardTemplate {
    pub fn new(
        card_id: CardId,
        name: impl Into<String>,
        attack: i32,
        health: i32,
        tavern_tier: u32,
    ) -> Self {
        Self {
            card_id,
            name: name.into(),
            attack,
            health,
            tavern_tier,
            tribe: Tribe::None,
            taunt: false,
            divine_shield: false,
            windfury: false,
            reborn: false,
            venomous: false,
            stealth: false,
            magnetic: false,
            intrinsic_golden: false,
            activate_cost: None,
        }
    }

    pub fn with_tribe(mut self, tribe: Tribe) -> Self {
        self.tribe = tribe;
        self
    }

    pub fn with_keyword(mut self, kw: Keyword) -> Self {
        match kw {
            Keyword::Taunt => self.taunt = true,
            Keyword::DivineShield => self.divine_shield = true,
            Keyword::Windfury => self.windfury = true,
            Keyword::Reborn => self.reborn = true,
            Keyword::Venomous => self.venomous = true,
            Keyword::Stealth => self.stealth = true,
            Keyword::Magnetic => self.magnetic = true,
        }
        self
    }

    pub fn with_intrinsic_golden(mut self) -> Self {
        self.intrinsic_golden = true;
        self
    }

    pub fn with_activate_cost(mut self, cost: u32) -> Self {
        self.activate_cost = Some(cost);
        self
    }

    /// Return the printed keywords on this template.
    pub fn keywords(&self) -> Vec<Keyword> {
        let mut kws = Vec::new();
        if self.taunt {
            kws.push(Keyword::Taunt);
        }
        if self.divine_shield {
            kws.push(Keyword::DivineShield);
        }
        if self.windfury {
            kws.push(Keyword::Windfury);
        }
        if self.reborn {
            kws.push(Keyword::Reborn);
        }
        if self.venomous {
            kws.push(Keyword::Venomous);
        }
        if self.stealth {
            kws.push(Keyword::Stealth);
        }
        if self.magnetic {
            kws.push(Keyword::Magnetic);
        }
        kws
    }

    /// Instantiate a fresh [`Unit`] from this template.
    pub fn instantiate(&self) -> Unit {
        let mut u = Unit::new(self.name.clone(), self.attack, self.health)
            .with_card_id(self.card_id)
            .with_tavern_tier(self.tavern_tier)
            .with_tribe(self.tribe);
        u.taunt = self.taunt;
        u.divine_shield = self.divine_shield;
        u.inherent_divine_shield = self.divine_shield;
        u.windfury = self.windfury;
        u.reborn = self.reborn;
        u.venomous = self.venomous;
        u.stealth = self.stealth;
        u.magnetic = self.magnetic;
        if self.intrinsic_golden {
            u.is_golden = true;
            u.intrinsic_golden = true;
        }
        init_unit_turn_charges(&mut u);
        u
    }
}

/// Initialize or reset per-turn charges (`Malchezaar`, `Thorned Trailblazer`) on `unit`.
pub fn init_unit_turn_charges(unit: &mut Unit) {
    tier3::malchezaar_prince_of_dance::reset_turn_charges(unit);
    tier3::thorned_trailblazer::reset_turn_charges(unit);
}

/// All 21 active Solo Tier 1 minions (Patch 36.6.3, excluding rotated Naga & Dark Paradox).
pub fn tier1_catalog() -> Vec<CardTemplate> {
    tier1::catalog()
}

/// Alias for [`tier1_catalog`].
pub fn solo_tier_1_catalog() -> Vec<CardTemplate> {
    tier1_catalog()
}

/// All 34 active Solo Tier 2 minions (Patch 36.6.3, including the 3 Volumizers).
pub fn tier2_catalog() -> Vec<CardTemplate> {
    tier2::catalog()
}

/// Alias for [`tier2_catalog`].
pub fn solo_tier_2_catalog() -> Vec<CardTemplate> {
    tier2_catalog()
}

/// All 43 active Solo Tier 3 minions (Patch 36.6.3).
pub fn tier3_catalog() -> Vec<CardTemplate> {
    tier3::catalog()
}

/// Alias for [`tier3_catalog`].
pub fn solo_tier_3_catalog() -> Vec<CardTemplate> {
    tier3_catalog()
}

/// Full active catalog (Solo Tier 1 + Solo Tier 2 + Solo Tier 3 = 98 minions).
pub fn full_catalog() -> Vec<CardTemplate> {
    let mut cards = tier1_catalog();
    cards.extend(tier2_catalog());
    cards.extend(tier3_catalog());
    cards
}

/// Alias for [`full_catalog`].
pub fn solo_full_catalog() -> Vec<CardTemplate> {
    full_catalog()
}

/// Minimal test catalog for generic economy scenarios.
pub fn default_test_catalog() -> Vec<CardTemplate> {
    tier1_catalog()
}

/// Resolve a catalog name to its templates.
pub fn catalog_for(name: &str) -> Result<Vec<CardTemplate>, String> {
    match name {
        "tier1" | "solo_tier_1" | "test" => Ok(tier1_catalog()),
        "tier2" | "solo_tier_2" => Ok(tier2_catalog()),
        "tier3" | "solo_tier_3" => Ok(tier3_catalog()),
        "tier1_2" => {
            let mut c = tier1_catalog();
            c.extend(tier2_catalog());
            Ok(c)
        }
        "full" | "solo_full" | "tier1_3" => Ok(full_catalog()),
        other => Err(format!(
            "unknown catalog {other:?}; expected one of: tier1, solo_tier_1, tier2, solo_tier_2, tier3, solo_tier_3, tier1_2, full, solo_full, test"
        )),
    }
}

// ---------------------------------------------------------------------------
// Unified Tier-Agnostic Tavern & Combat Hook Dispatch
// ---------------------------------------------------------------------------

/// Run stat-threshold checks whenever a unit's stats change (in Tavern or Combat).
pub fn check_stat_thresholds(unit: &mut Unit) {
    if unit.card_id == tier1::scarlet_survivor::ID {
        tier1::scarlet_survivor::check_threshold(unit);
    }
}

/// Synchronize a unit's persistent "wherever this is" auras (`Undead` attack, `Eternal Knight`, `Volumizers`, `Relentless Deflector`).
pub fn sync_unit_auras(unit: &mut Unit, auras: &PlayerAuras) {
    if unit.is_spell {
        return;
    }
    tier2::nerubian_deathswarmer::sync_unit_undead_attack(unit, auras);
    tier2::eternal_knight::sync_unit(unit, auras);
    tier3::relentless_deflector::sync_taunt(unit);
    if matches!(
        unit.card_id,
        tier2::blue_volumizer::ID | tier2::green_volumizer::ID | tier2::red_volumizer::ID
    ) {
        let (app_atk, app_hp) = unit.volumizer_stacks_applied;
        let d_atk = auras.volumizer_bonus_atk - app_atk;
        let d_hp = auras.volumizer_bonus_hp - app_hp;
        if d_atk != 0 || d_hp != 0 {
            unit.volumizer_stacks_applied = (auras.volumizer_bonus_atk, auras.volumizer_bonus_hp);
            unit.add_stats(d_atk, d_hp);
        }
    }
}

/// Apply on-play Battlecry / Choose-One when `unit` is played from `hand` onto `board` at `board_pos`.
pub fn on_play_battlecry(
    state: &mut TavernState,
    unit: &mut Unit,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    match unit.card_id {
        tier1::joyous::ID => tier1::joyous::on_battlecry(state, unit),
        tier1::ominous_seer::ID => tier1::ominous_seer::on_battlecry(state, unit),
        tier1::dune_dweller::ID => tier1::dune_dweller::on_battlecry(state, unit),
        tier1::bubble_gunner::ID => tier1::bubble_gunner::on_battlecry(unit, rng),
        tier1::southsea_busker::ID => tier1::southsea_busker::on_battlecry(state, unit),
        tier1::razorfen_geomancer::ID => tier1::razorfen_geomancer::on_battlecry(state, unit),
        tier2::bilgewater_breakout::ID => {
            tier2::bilgewater_breakout::on_battlecry(state, unit, rng)
        }
        tier2::crater_miner::ID => tier2::crater_miner::on_battlecry(state, unit, pool, rng),
        tier2::electric_synthesizer::ID => tier2::electric_synthesizer::on_battlecry(state, unit),
        tier2::forest_rover::ID => tier2::forest_rover::on_battlecry(state, unit),
        tier2::intrepid_botanist::ID => {
            tier2::intrepid_botanist::on_battlecry(state, unit, pool, rng)
        }
        tier2::laboratory_assistant::ID => tier2::laboratory_assistant::on_battlecry(state, unit),
        tier2::mind_muck::ID => tier2::mind_muck::on_battlecry(state, unit, pool, rng),
        tier2::nerubian_deathswarmer::ID => {
            tier2::nerubian_deathswarmer::on_battlecry(state, unit)
        }
        tier3::auto_accelerator::ID => tier3::auto_accelerator::on_battlecry(state, unit, rng),
        tier3::azsharan_cutlassier::ID => tier3::azsharan_cutlassier::on_battlecry(state, unit),
        tier3::disguised_graverobber::ID => {
            tier3::disguised_graverobber::on_battlecry(state, unit, board_pos, pool)
        }
        tier3::fearless_foodie::ID => {
            tier3::fearless_foodie::on_battlecry(state, unit, pool, rng)
        }
        tier3::fetid_corroder::ID => tier3::fetid_corroder::on_battlecry(state, unit),
        tier3::iron_groundskeeper::ID => tier3::iron_groundskeeper::on_battlecry(state, unit),
        tier3::sly_infiltrator::ID => {
            tier3::sly_infiltrator::on_battlecry(state, unit, pool, rng)
        }
        tier3::sprightly_scarab::ID => {
            tier3::sprightly_scarab::on_battlecry(state, unit, board_pos, pool, rng)
        }
        tokens::TOKEN_BLUE_CHROMADRAKE
        | tokens::TOKEN_BLACK_CHROMADRAKE
        | tokens::TOKEN_GREEN_CHROMADRAKE
        | tokens::TOKEN_BRONZE_CHROMADRAKE
        | tokens::TOKEN_RED_CHROMADRAKE => {
            tier3::hired_mount::on_chromadrake_battlecry(state, unit, rng)
        }
        _ => {}
    }
}

/// Apply first-time play or Magnetize triggers (`Blue`/`Green`/`Red Volumizer`).
pub fn on_first_play_or_magnetize(state: &mut TavernState, unit: &mut Unit) {
    match unit.card_id {
        tier2::blue_volumizer::ID => {
            tier2::blue_volumizer::on_first_play_or_magnetize(state, unit)
        }
        tier2::green_volumizer::ID => {
            tier2::green_volumizer::on_first_play_or_magnetize(state, unit)
        }
        tier2::red_volumizer::ID => tier2::red_volumizer::on_first_play_or_magnetize(state, unit),
        _ => {}
    }
}

/// Transfer card-specific properties (`Lullabot`, `Accord-o-Tron`, Blood Gems) when `source` is Magnetized onto `target`.
pub fn on_magnetize_transfer(source: &Unit, target: &mut Unit) {
    target.eot_health_bonus += source.eot_health_bonus;
    if source.card_id == tier1::lullabot::ID {
        target.eot_health_bonus += if source.is_golden { 2 } else { 1 };
    }
    target.sot_gold_bonus += source.sot_gold_bonus;
    if source.card_id == tier3::accord_o_tron::ID {
        target.sot_gold_bonus += if source.is_golden { 2 } else { 1 };
    }
    target.blood_gems_played += source.blood_gems_played;
    target.blood_gem_stats_applied.0 += source.blood_gem_stats_applied.0;
    target.blood_gem_stats_applied.1 += source.blood_gem_stats_applied.1;
}

/// Apply board-wide observers after a minion is played (`was_magnetized = false`) or Magnetized (`was_magnetized = true`).
pub fn after_play_minion(
    state: &mut TavernState,
    _played_card_id: CardId,
    played_tribe: Tribe,
    board_pos: usize,
    was_magnetized: bool,
) {
    if !was_magnetized {
        tier1::wrath_weaver::after_play_minion(state, played_tribe, board_pos);
    }
    tier2::mechagnome_interpreter::after_play_or_magnetize_mech(
        state,
        played_tribe,
        board_pos,
        was_magnetized,
    );
}

/// Apply On-Sell triggers when `sold` is sold from `board`.
pub fn on_sell(state: &mut TavernState, sold: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    match sold.card_id {
        tier1::zoatroid::ID => tier1::zoatroid::on_sell(state, sold),
        tier2::fire_baller::ID => tier2::fire_baller::on_sell(state, sold),
        tier2::patient_scout::ID => tier2::patient_scout::on_sell(state, sold, pool, rng),
        tier2::sellemental::ID => tier2::sellemental::on_sell(state, sold),
        tier2::snow_baller::ID => tier2::snow_baller::on_sell(state, sold),
        tier2::tad::ID => tier2::tad::on_sell(state, sold, pool, rng),
        tier2::wandering_willbreaker::ID => tier2::wandering_willbreaker::on_sell(state, sold, rng),
        tier3::greedy_conniver::ID => tier3::greedy_conniver::on_sell(state, sold, pool, rng),
        tier3::shoalfin_mystic::ID => tier3::shoalfin_mystic::on_sell(state, sold),
        _ => {}
    }
}

/// Apply Start-of-Turn triggers on a minion on `board` (`Patient Scout`, `Malchezaar`, `Thorned Trailblazer`).
pub fn on_start_turn(unit: &mut Unit) {
    if unit.card_id == tier2::patient_scout::ID {
        tier2::patient_scout::on_start_turn(unit);
    }
    init_unit_turn_charges(unit);
}

/// Apply board-wide Start-of-Turn triggers (`Accord-o-Tron`).
pub fn on_start_turn_board(state: &mut TavernState) {
    tier3::accord_o_tron::on_start_turn(state);
}

/// Apply End-of-Turn triggers across `board` and `hand` when `EndTurn` is taken.
pub fn on_end_turn(state: &mut TavernState) {
    tier1::lullabot::on_end_turn(state);
    tier2::surfing_sylvar::on_end_turn(state);
    tier3::gem_rat::on_end_turn(state);
    tier3::trench_fighter::on_end_turn(state);
}

/// Apply board-wide observers when a Tavern spell is cast (`Timecap'n Hooktail`, `Vicious Mindslasher`).
pub fn on_cast_tavern_spell(state: &mut TavernState) {
    tier3::timecapn_hooktail::on_cast_tavern_spell(state);
    tier3::vicious_mindslasher::on_cast_tavern_spell(state);
}

/// Resolve discard triggers when `discarded` is discarded from hand (`Sludge Corrosion`).
pub fn on_discard_hand_card(
    state: &mut TavernState,
    discarded: &Unit,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if discarded.card_id == tokens::SPELL_SLUDGE_CORROSION {
        for _ in 0..2 {
            state.auras.spells_played += 1;
            spells::cast_spell(state, tokens::make_sludge_corrosion(), 0, pool, rng);
        }
    }
}

/// Returns `true` if any minion on `board` rewinds hero damage (`Soul Rewinder`).
pub fn board_prevents_hero_damage(board: &[Unit]) -> bool {
    board.iter().any(|u| u.card_id == tier2::soul_rewinder::ID)
}

/// Trigger hero-damage observers (`Soul Rewinder`), returning `true` if the damage was rewound.
pub fn on_hero_damage_taken(board: &mut [Unit]) -> bool {
    tier2::soul_rewinder::on_hero_damage_taken(board)
}

/// Returns the Gold cost of a minion's `Activate` ability, if it has one.
pub fn activate_cost(card_id: CardId) -> Option<u32> {
    match card_id {
        tier1::suspicious_prisonguard::ID => Some(tier1::suspicious_prisonguard::ACTIVATE_COST),
        tier2::brain_rotter::ID => Some(tier2::brain_rotter::ACTIVATE_COST),
        tier2::clever_castaway::ID => Some(tier2::clever_castaway::ACTIVATE_COST),
        tier2::decoy_conjurer::ID => Some(tier2::decoy_conjurer::ACTIVATE_COST),
        tier2::lurking_lionfish::ID => Some(tier2::lurking_lionfish::ACTIVATE_COST),
        tier3::abyssal_envoy::ID => Some(tier3::abyssal_envoy::ACTIVATE_COST),
        tier3::fruit_vendor::ID => Some(tier3::fruit_vendor::ACTIVATE_COST),
        tier3::hired_mount::ID => Some(tier3::hired_mount::ACTIVATE_COST),
        tier3::mangled_bandit::ID => Some(tier3::mangled_bandit::ACTIVATE_COST),
        _ => None,
    }
}

/// Returns the target domain required by a minion's `Activate` ability.
pub fn activate_target_kind(card_id: CardId) -> ActivateTargetKind {
    match card_id {
        tier1::suspicious_prisonguard::ID => ActivateTargetKind::BoardOther,
        tier2::brain_rotter::ID | tier3::abyssal_envoy::ID | tier3::mangled_bandit::ID => {
            ActivateTargetKind::HandCard
        }
        tier2::lurking_lionfish::ID => ActivateTargetKind::ShopCard,
        _ => ActivateTargetKind::None,
    }
}

/// Execute a minion's `Activate` ability on `board`.
pub fn on_activate(
    state: &mut TavernState,
    source_pos: usize,
    target_pos: Option<usize>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    match state.board[source_pos].card_id {
        tier1::suspicious_prisonguard::ID => {
            tier1::suspicious_prisonguard::on_activate(state, source_pos, target_pos)
        }
        tier2::brain_rotter::ID => {
            tier2::brain_rotter::on_activate(state, source_pos, target_pos, pool, rng)
        }
        tier2::clever_castaway::ID => tier2::clever_castaway::on_activate(state, source_pos, rng),
        tier2::decoy_conjurer::ID => tier2::decoy_conjurer::on_activate(state, source_pos),
        tier2::lurking_lionfish::ID => {
            tier2::lurking_lionfish::on_activate(state, source_pos, target_pos, pool)
        }
        tier3::abyssal_envoy::ID => {
            tier3::abyssal_envoy::on_activate(state, source_pos, target_pos, pool, rng)
        }
        tier3::fruit_vendor::ID => tier3::fruit_vendor::on_activate(state, source_pos),
        tier3::hired_mount::ID => tier3::hired_mount::on_activate(state, source_pos, rng),
        tier3::mangled_bandit::ID => {
            tier3::mangled_bandit::on_activate(state, source_pos, target_pos, pool, rng)
        }
        _ => {}
    }
}

/// Apply Start-of-Combat hand summons (`Flighty Scout`).
pub fn on_start_of_combat_hand(hand: &[Unit], board: &mut Vec<Unit>) {
    tier1::flighty_scout::apply_start_of_combat_hand(hand, board);
}

/// Resolve Start-of-Combat board triggers for one side (`Electric Synthesizer`, `Humming Bird`, `Amber Guardian`, `Diremuck Forager`).
#[allow(clippy::too_many_arguments)]
pub fn on_start_of_combat(
    side: Side,
    board: &mut Vec<Unit>,
    auras: &PlayerAuras,
    hand: &[Unit],
    hand_summoned: &mut [bool],
    combat_beast_bonus_atk: &mut i32,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    let sources: Vec<(UnitId, CardId, bool)> = board
        .iter()
        .map(|u| (u.id, u.card_id, u.is_golden))
        .collect();
    for (id, card_id, is_golden) in sources {
        if !board.iter().any(|u| u.id == id && u.health > 0) {
            continue;
        }
        match card_id {
            tier2::electric_synthesizer::ID => {
                tier2::electric_synthesizer::on_start_of_combat(side, board, id, is_golden, events);
            }
            tier2::humming_bird::ID => {
                tier2::humming_bird::on_start_of_combat(
                    side,
                    board,
                    combat_beast_bonus_atk,
                    is_golden,
                    events,
                );
            }
            tier3::amber_guardian::ID => {
                tier3::amber_guardian::on_start_of_combat(
                    side, board, id, is_golden, rng, events,
                );
            }
            tier3::diremuck_forager::ID => {
                tier3::diremuck_forager::on_start_of_combat(
                    side,
                    board,
                    id,
                    is_golden,
                    auras,
                    hand,
                    hand_summoned,
                    *combat_beast_bonus_atk,
                    next_id,
                    events,
                );
            }
            _ => {}
        }
    }
}

/// Resolve a minion's On-Attack (`Rally`) effect during combat.
/// Returns any token(s) to be summoned immediately to the attacker's right.
#[allow(clippy::too_many_arguments)]
pub fn on_rally(
    side: Side,
    board: &mut [Unit],
    attacker_pos: usize,
    auras: &mut PlayerAuras,
    hand: &[Unit],
    hand_summoned: &mut [bool],
    generated_hand: &mut Vec<Unit>,
    events: &mut Vec<Event>,
) -> Vec<Unit> {
    let card_id = board[attacker_pos].card_id;
    let attacker_id = board[attacker_pos].id;
    let is_golden = board[attacker_pos].is_golden;
    match card_id {
        tier1::flittering_bat::ID => tier1::flittering_bat::on_rally(&mut board[attacker_pos]),
        tier1::glim_guardian::ID => tier1::glim_guardian::on_rally(&mut board[attacker_pos]),
        tier1::tusked_camper::ID => {
            tier1::tusked_camper::on_rally(&mut board[attacker_pos], auras)
        }
        tier2::expert_aviator::ID => {
            tier2::expert_aviator::on_rally(&board[attacker_pos], hand, hand_summoned)
        }
        tier2::roadboar::ID => {
            tier2::roadboar::on_rally(&board[attacker_pos], generated_hand)
        }
        tier3::blue_whelp::ID => {
            tier3::blue_whelp::on_rally(&board[attacker_pos], auras);
            Vec::new()
        }
        tier3::wolf_pup::ID => {
            tier3::wolf_pup::on_rally(side, board, attacker_id, is_golden, events);
            Vec::new()
        }
        _ => Vec::new(),
    }
}

/// Resolve board-wide observers when a friendly minion attacks (`Prodigious Tusker`, `Roaring Recruiter`).
pub fn on_friendly_attack(
    side: Side,
    board: &mut [Unit],
    attacker_id: UnitId,
    auras: &PlayerAuras,
    events: &mut Vec<Event>,
) {
    tier2::prodigious_tusker::on_friendly_attack(side, board, attacker_id, auras, events);
    tier3::roaring_recruiter::on_friendly_attack(side, board, attacker_id, events);
}

/// Resolve on-damage-dealt triggers (`Treasure Parrot`, `Devout Hellcaller`).
pub fn on_damage_dealt(
    side: Side,
    board: &mut [Unit],
    source_id: UnitId,
    amount: i32,
    hand: &mut Vec<Unit>,
    hand_summoned: &mut Vec<bool>,
    events: &mut Vec<Event>,
) {
    if amount <= 0 {
        return;
    }
    let Some(src_idx) = board.iter().position(|u| u.id == source_id) else {
        return;
    };
    let src_tribe = board[src_idx].tribe;
    tier3::treasure_parrot::on_dealt_damage(
        &mut board[src_idx],
        amount,
        hand,
        hand_summoned,
    );
    tier3::devout_hellcaller::on_friendly_dealt_damage(
        side, board, source_id, src_tribe, events,
    );
}

/// Returns `true` if `card_id` deals excess attack damage to adjacent enemy(ies) (`Wildfire Elemental`).
pub fn deals_excess_damage_to_neighbors(card_id: CardId) -> bool {
    card_id == tier3::wildfire_elemental::ID
}

/// Resolve on-damage-taken triggers (`Very Hungry Winterfinner`).
pub fn on_damage_taken(unit: &Unit, hand: &mut [Unit], rng: &mut Rng) {
    tier2::very_hungry_winterfinner::on_damage_taken(unit, hand, rng);
}

/// Update persistent player aura counters and Avenge triggers when a friendly minion dies (`Eternal Knight`, `Relentless Deflector`).
pub fn on_unit_died(unit: &Unit, surviving_board: &mut [Unit], auras: &mut PlayerAuras) {
    if unit.card_id == tier2::eternal_knight::ID {
        auras.eternal_knights_died += 1;
    }
    for survivor in surviving_board.iter_mut() {
        tier3::relentless_deflector::on_friendly_death(survivor);
    }
}

/// Resolve a dying minion's `Deathrattle` (token summons, board buffs, aura scaling, and hand generation) via [`DeathrattleContext`].
pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    match dying.card_id {
        deities::CARD_YSHAARJ => {
            for token in
                deities::yshaarj_deathrattle_summons(dying.is_golden, ctx.dead_aberrations)
            {
                ctx.summon(dying.id, token);
            }
        }
        tier1::buzzing_vermin::ID => tier1::buzzing_vermin::on_deathrattle(dying, ctx),
        tier1::cord_puller::ID => tier1::cord_puller::on_deathrattle(dying, ctx),
        tier1::harmless_bonehead::ID => tier1::harmless_bonehead::on_deathrattle(dying, ctx),
        tier2::forest_rover::ID => tier2::forest_rover::on_deathrattle(dying, ctx),
        tier2::scarlet_skull::ID => tier2::scarlet_skull::on_deathrattle(dying, ctx),
        tier2::underrot_spawn::ID => tier2::underrot_spawn::on_deathrattle(dying, ctx),
        tier3::cadaver_caretaker::ID => tier3::cadaver_caretaker::on_deathrattle(dying, ctx),
        tier3::drifting_sacrifice::ID => tier3::drifting_sacrifice::on_deathrattle(dying, ctx),
        tier3::handless_forsaken::ID => tier3::handless_forsaken::on_deathrattle(dying, ctx),
        tier3::locked_up_mutineer::ID => tier3::locked_up_mutineer::on_deathrattle(dying, ctx),
        tier3::mummifier::ID => tier3::mummifier::on_deathrattle(dying, ctx),
        tier3::rescue_bot::ID => tier3::rescue_bot::on_deathrattle(dying, ctx),
        tier3::tasty_lobster::ID => tier3::tasty_lobster::on_deathrattle(dying, ctx),
        tier3::trapped_clapper::ID => tier3::trapped_clapper::on_deathrattle(dying, ctx),
        tier3::unwilling_slacker::ID => tier3::unwilling_slacker::on_deathrattle(dying, ctx),
        tier3::waveling::ID => tier3::waveling::on_deathrattle(dying, ctx),
        _ => {}
    }
}

/// Synchronize dynamic combat/player auras across `board` and `hand` after deaths resolve.
pub fn sync_combat_auras(
    board: &mut [Unit],
    hand: &mut [Unit],
    auras: &PlayerAuras,
    _friendly_deaths_this_combat: u32,
) {
    for u in board.iter_mut() {
        sync_unit_auras(u, auras);
    }
    for h in hand.iter_mut() {
        sync_unit_auras(h, auras);
    }
}

/// Apply post-combat persistence from a combat unit back to its Tavern counterpart (`Tarecgosa`, `Devout Hellcaller`, `Treasure Parrot`).
pub fn on_post_combat_unit(
    pre_combat: &Unit,
    post_combat_board: &[Unit],
    tavern_unit: &mut Unit,
) {
    if tavern_unit.card_id == tier2::tarecgosa::ID {
        tier2::tarecgosa::apply_post_combat_persistence(
            pre_combat,
            post_combat_board,
            tavern_unit,
        );
    }
    if let Some(post) = post_combat_board.iter().find(|u| u.id == pre_combat.id) {
        if post.perm_atk_gained != 0 || post.perm_hp_gained != 0 {
            tavern_unit.add_stats(post.perm_atk_gained, post.perm_hp_gained);
        }
        if tavern_unit.card_id == tier3::treasure_parrot::ID {
            tavern_unit.damage_dealt_counter = post.damage_dealt_counter;
            tavern_unit.threshold_triggered = post.threshold_triggered;
        }
    }
}
