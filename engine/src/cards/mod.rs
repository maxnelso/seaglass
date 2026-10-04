//! Card templates, catalogs, and unified tier-agnostic per-card hook dispatch.
//!
//! Card definitions are organized by folder (`src/cards/tier1/`, `src/cards/tier2/`,
//! `src/cards/tier3/`, `src/cards/tier4/`, `src/cards/tier5/`, `src/cards/deities.rs`,
//! `src/cards/spells.rs`, `src/cards/tokens.rs`), while all Tavern and Combat hooks are
//! dispatched uniformly by `CardId` in this module without any tier-specific branching.

pub mod deities;
pub mod spells;
pub mod tier1;
pub mod tier2;
pub mod tier3;
pub mod tier4;
pub mod tier5;
pub mod tokens;

use crate::combat::MAX_BOARD_SIZE;
use crate::events::Event;
use crate::model::{CardId, Keyword, PlayerAuras, Side, Tribe, Unit, UnitId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Target domain required by a minion's `Activate` ability (`docs/tavern.md` §5.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActivateTargetKind {
    /// Untargeted (`Clever Castaway`, `Decoy Conjurer`, `Fruit Vendor`, `Hired Mount`, `Drone Duplicator`, `Living Prison`, `Sacrificial Wrathguard`, `Soulkeeping Jailer`, `Deft Deserter`).
    None,
    /// Requires another friendly minion on `board` (`Suspicious Prisonguard`).
    BoardOther,
    /// Requires a different friendly Undead minion on `board` (`Dead Bellringer`).
    BoardOtherUndead,
    /// Requires a different friendly Murloc minion on `board` (`Sewer Escapee`).
    BoardOtherMurloc,
    /// Requires any friendly minion on `board` (`Kelp Keeper`, `Sky-hatch Runaway`).
    BoardAny,
    /// Requires a card in `hand` (`Brain Rotter`, `Abyssal Envoy`, `Mangled Bandit`, `Mindbending Recruiter`, `N'raqi Frostcaller`).
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
    pub in_combat: bool,
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
        if self.in_combat {
            apply_combat_summon_modifiers(
                self.board,
                self.auras,
                self.combat_beast_bonus_atk,
                token.id,
                &mut token,
            );
        } else {
            sync_unit_auras(&mut token, self.auras);
            token.sync_max_stats();
            check_stat_thresholds(&mut token);
        }
        self.events.push(Event::UnitSummoned {
            side: self.side,
            source: source_id,
            unit: token.id,
            name: token.name.clone(),
            attack: token.attack,
            health: token.health,
            reason: "Deathrattle",
        });
        let pos = *self.cursor;
        self.board.insert(pos, token);
        *self.cursor += 1;
        if !self.in_combat {
            tier5::lurking_leviathan::on_beast_summoned_tavern(self.board, pos);
        }
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
    pub fn add_to_hand(&mut self, mut card: Unit) {
        if self.hand.len() < 10 {
            sync_unit_auras(&mut card, self.auras);
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
        tier4::enchanted_sentinel::init_spell_aura(&mut u);
        tier4::humongozz::init_spell_aura(&mut u);
        u
    }
}

/// Initialize or reset per-turn charges (`Malchezaar`, `Thorned Trailblazer`, `Drone Duplicator`, `Living Prison`) on `unit`.
pub fn init_unit_turn_charges(unit: &mut Unit) {
    tier3::malchezaar_prince_of_dance::reset_turn_charges(unit);
    tier3::thorned_trailblazer::reset_turn_charges(unit);
    unit.extra_magnetize_this_turn = 0;
    unit.living_prison_stacks = 0;
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

/// All 58 active Solo Tier 4 minions (Patch 36.6.3).
pub fn tier4_catalog() -> Vec<CardTemplate> {
    tier4::catalog()
}

/// Alias for [`tier4_catalog`].
pub fn solo_tier_4_catalog() -> Vec<CardTemplate> {
    tier4_catalog()
}

/// All 52 active Solo Tier 5 minions (Patch 36.6.3).
pub fn tier5_catalog() -> Vec<CardTemplate> {
    tier5::catalog()
}

/// Alias for [`tier5_catalog`].
pub fn solo_tier_5_catalog() -> Vec<CardTemplate> {
    tier5_catalog()
}

/// Full active catalog (Solo Tier 1 + Tier 2 + Tier 3 + Tier 4 + Tier 5 = 208 minions).
pub fn full_catalog() -> Vec<CardTemplate> {
    let mut cards = tier1_catalog();
    cards.extend(tier2_catalog());
    cards.extend(tier3_catalog());
    cards.extend(tier4_catalog());
    cards.extend(tier5_catalog());
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
        "tier4" | "solo_tier_4" => Ok(tier4_catalog()),
        "tier5" | "solo_tier_5" => Ok(tier5_catalog()),
        "tier1_2" => {
            let mut c = tier1_catalog();
            c.extend(tier2_catalog());
            Ok(c)
        }
        "tier1_3" => {
            let mut c = tier1_catalog();
            c.extend(tier2_catalog());
            c.extend(tier3_catalog());
            Ok(c)
        }
        "tier1_4" => {
            let mut c = tier1_catalog();
            c.extend(tier2_catalog());
            c.extend(tier3_catalog());
            c.extend(tier4_catalog());
            Ok(c)
        }
        "full" | "solo_full" | "tier1_5" => Ok(full_catalog()),
        other => Err(format!(
            "unknown catalog {other:?}; expected one of: tier1, solo_tier_1, tier2, solo_tier_2, tier3, solo_tier_3, tier4, solo_tier_4, tier5, solo_tier_5, tier1_2, tier1_3, tier1_4, tier1_5, full, solo_full, test"
        )),
    }
}

/// Instantiate a plain (non-Golden, unbuffed) copy of `unit`.
pub fn instantiate_plain_copy(unit: &Unit) -> Unit {
    if let Some(tpl) = full_catalog().into_iter().find(|t| t.card_id == unit.card_id) {
        tpl.instantiate()
    } else {
        let mut copy = unit.clone();
        copy.is_golden = copy.intrinsic_golden;
        copy.attack = copy.base_attack;
        copy.health = copy.base_health;
        copy.max_attack = copy.base_attack;
        copy.max_health = copy.base_health;
        copy
    }
}

/// Returns `true` if `card_id` is a Battlecry minion (`Brann Bronzebeard`, `Kalecgos`, `Hired Headhunter`).
pub fn is_battlecry_minion(card_id: CardId) -> bool {
    matches!(
        card_id,
        tier1::joyous::ID
            | tier1::ominous_seer::ID
            | tier1::dune_dweller::ID
            | tier1::bubble_gunner::ID
            | tier1::southsea_busker::ID
            | tier1::razorfen_geomancer::ID
            | tier2::bilgewater_breakout::ID
            | tier2::electric_synthesizer::ID
            | tier2::forest_rover::ID
            | tier2::laboratory_assistant::ID
            | tier2::mind_muck::ID
            | tier2::nerubian_deathswarmer::ID
            | tier3::auto_accelerator::ID
            | tier3::azsharan_cutlassier::ID
            | tier3::disguised_graverobber::ID
            | tier3::fetid_corroder::ID
            | tier3::iron_groundskeeper::ID
            | tier4::en_djinn_blazer::ID
            | tier4::gormling_gourmet::ID
            | tier4::imposing_percussionist::ID
            | tier4::leyline_surfacer::ID
            | tier4::lovesick_balladist::ID
            | tier4::maw_caster::ID
            | tier4::razorfen_flapper::ID
            | tier4::refreshing_anomaly::ID
            | tier4::tavern_tempest::ID
            | tier5::draconic_warden::ID
            | tier5::elite_navigator::ID
            | tier5::firelands_fugitive::ID
            | tier5::firescale_hoarder::ID
            | tier5::hackerfin::ID
            | tier5::nightmare_par_tea_guest::ID
            | tier5::nraqi_sapper::ID
            | tier5::primalfin_lookout::ID
            | tier5::rodeo_performer::ID
            | tier5::shipwrecked_rascal::ID
            | tokens::TOKEN_BLUE_CHROMADRAKE
            | tokens::TOKEN_BLACK_CHROMADRAKE
            | tokens::TOKEN_GREEN_CHROMADRAKE
            | tokens::TOKEN_BRONZE_CHROMADRAKE
            | tokens::TOKEN_RED_CHROMADRAKE
    )
}

/// Returns `true` if `card_id` is a Deathrattle minion (`Contracted Corpse`, `Ghastcoiler`).
pub fn is_deathrattle_minion(card_id: CardId) -> bool {
    matches!(
        card_id,
        tier1::buzzing_vermin::ID
            | tier1::cord_puller::ID
            | tier1::harmless_bonehead::ID
            | tier2::forest_rover::ID
            | tier2::scarlet_skull::ID
            | tier2::underrot_spawn::ID
            | tier3::cadaver_caretaker::ID
            | tier3::drifting_sacrifice::ID
            | tier3::handless_forsaken::ID
            | tier3::locked_up_mutineer::ID
            | tier3::mummifier::ID
            | tier3::rescue_bot::ID
            | tier3::tasty_lobster::ID
            | tier3::trapped_clapper::ID
            | tier3::unwilling_slacker::ID
            | tier3::waveling::ID
            | tier4::conveyor_construct::ID
            | tier4::friendly_geist::ID
            | tier4::gormling_gourmet::ID
            | tier4::imp_lusionist::ID
            | tier4::leyline_surfacer::ID
            | tier4::plaguerunner::ID
            | tier4::razorfen_flapper::ID
            | tier4::sacrificial_wrathguard::ID
            | tier5::draconic_warden::ID
            | tier5::eternal_summoner::ID
            | tier5::faceless_converter::ID
            | tier5::firescale_hoarder::ID
            | tier5::ghastcoiler::ID
            | tier5::goldrinn_the_great_wolf::ID
            | tier5::leeroy_the_reckless::ID
            | tier5::nightmare_par_tea_guest::ID
            | tier5::nraqi_sapper::ID
            | tier5::sewer_lord::ID
            | tier5::ship_master_eudora::ID
            | tier5::shipwrecked_rascal::ID
            | tier5::turquoise_skitterer::ID
            | tokens::TOKEN_SEWER_RAT
            | deities::CARD_YSHAARJ
    )
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

/// Synchronize a unit's persistent "wherever this is" auras (`Undead` attack, `Eternal Knight`, `Volumizers`, `Relentless Deflector`, `Holy Vanguard`, `Maritime Extortionist`).
pub fn sync_unit_auras(unit: &mut Unit, auras: &PlayerAuras) {
    if unit.is_spell {
        return;
    }
    tier2::nerubian_deathswarmer::sync_unit_undead_attack(unit, auras);
    tier2::eternal_knight::sync_unit(unit, auras);
    tier3::relentless_deflector::sync_taunt(unit);
    tier4::holy_vanguard::sync_unit(unit, auras);
    tier4::maritime_extortionist::sync_unit(unit, auras);
    tier4::enchanted_sentinel::init_spell_aura(unit);
    tier4::humongozz::init_spell_aura(unit);
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

/// Synchronize board-aura spell stat bonuses (`Enchanted Sentinel`, `Humon'gozz`) into `auras.spell_bonus_atk / hp`.
pub fn sync_board_spell_auras(board: &[Unit], auras: &mut PlayerAuras) {
    let total_atk: i32 = board.iter().map(|u| u.spell_atk_aura).sum();
    let total_hp: i32 = board.iter().map(|u| u.spell_hp_aura).sum();
    let (applied_atk, applied_hp) = auras.board_spell_bonus_applied;
    auras.spell_bonus_atk += total_atk - applied_atk;
    auras.spell_bonus_hp += total_hp - applied_hp;
    auras.board_spell_bonus_applied = (total_atk, total_hp);
}

/// Apply all combat summon modifiers (persistent auras, `Goldrinn`, `Humming Bird`, `Lurking Leviathan`, `Banana Slamma`, stat thresholds) to a newly summoned unit.
pub fn apply_combat_summon_modifiers(
    board: &mut [Unit],
    auras: &PlayerAuras,
    combat_beast_bonus_atk: i32,
    summoned_id: UnitId,
    token: &mut Unit,
) {
    sync_unit_auras(token, auras);
    if token.tribe.matches(Tribe::Beast) {
        if auras.goldrinn_bonus != 0 {
            token.add_stats(auras.goldrinn_bonus, auras.goldrinn_bonus);
        }
        if combat_beast_bonus_atk != 0 {
            token.add_stats(combat_beast_bonus_atk, 0);
        }
    }
    tier5::lurking_leviathan::on_beast_summoned_combat(board, summoned_id, token);
    tier4::banana_slamma::on_beast_summoned(board, summoned_id, token);
    token.sync_max_stats();
    check_stat_thresholds(token);
}

fn dispatch_single_play_battlecry(
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
        tier4::en_djinn_blazer::ID => tier4::en_djinn_blazer::on_battlecry(state, unit),
        tier4::gormling_gourmet::ID => tier4::gormling_gourmet::on_battlecry(state, unit),
        tier4::imposing_percussionist::ID => {
            tier4::imposing_percussionist::on_battlecry(state, unit, pool, rng)
        }
        tier4::leyline_surfacer::ID => tier4::leyline_surfacer::on_battlecry(state, unit),
        tier4::lovesick_balladist::ID => tier4::lovesick_balladist::on_battlecry(state, unit),
        tier4::maw_caster::ID => {
            tier4::maw_caster::on_battlecry(state, unit, board_pos, pool, rng)
        }
        tier4::razorfen_flapper::ID => tier4::razorfen_flapper::on_battlecry(state, unit),
        tier4::refreshing_anomaly::ID => tier4::refreshing_anomaly::on_battlecry(state, unit),
        tier4::snare_trapper::ID => {
            tier4::snare_trapper::on_battlecry(state, unit, pool, rng)
        }
        tier4::tavern_tempest::ID => {
            tier4::tavern_tempest::on_battlecry(state, unit, pool, rng)
        }
        tier5::draconic_warden::ID => tier5::draconic_warden::on_battlecry(state, unit, rng),
        tier5::elite_navigator::ID => tier5::elite_navigator::on_battlecry(state, unit, rng),
        tier5::firelands_fugitive::ID => tier5::firelands_fugitive::on_battlecry(state, unit),
        tier5::firescale_hoarder::ID => tier5::firescale_hoarder::on_battlecry(state, unit),
        tier5::hackerfin::ID => tier5::hackerfin::on_battlecry(state, unit),
        tier5::nightmare_par_tea_guest::ID => {
            tier5::nightmare_par_tea_guest::on_battlecry(state, unit)
        }
        tier5::nraqi_sapper::ID => tier5::nraqi_sapper::on_battlecry(state, unit),
        tier5::primalfin_lookout::ID => {
            tier5::primalfin_lookout::on_battlecry(state, unit, pool, rng)
        }
        tier5::rodeo_performer::ID => tier5::rodeo_performer::on_battlecry(state, unit, rng),
        tier5::shipwrecked_rascal::ID => tier5::shipwrecked_rascal::on_battlecry(state, unit, rng),
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

/// Apply on-play Battlecry / Choose-One when `unit` is played from `hand` onto `board` at `board_pos`.
pub fn on_play_battlecry(
    state: &mut TavernState,
    unit: &mut Unit,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if is_battlecry_minion(unit.card_id) {
        let repeats = tier5::brann_bronzebeard::battlecry_multiplier(&state.board);
        for _ in 0..repeats {
            dispatch_single_play_battlecry(state, unit, board_pos, pool, rng);
            tier5::kalecgos_arcane_aspect::after_battlecry_triggered(state, unit);
        }
    } else {
        dispatch_single_play_battlecry(state, unit, board_pos, pool, rng);
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

/// Transfer card-specific properties (`Lullabot`, `Accord-o-Tron`, `Enchanted Sentinel`, Blood Gems) when `source` is Magnetized onto `target`.
pub fn on_magnetize_transfer(source: &Unit, target: &mut Unit) {
    target.eot_health_bonus += source.eot_health_bonus;
    if source.card_id == tier1::lullabot::ID {
        target.eot_health_bonus += if source.is_golden { 2 } else { 1 };
    }
    target.sot_gold_bonus += source.sot_gold_bonus;
    if source.card_id == tier3::accord_o_tron::ID {
        target.sot_gold_bonus += if source.is_golden { 2 } else { 1 };
    }
    target.spell_atk_aura += source.spell_atk_aura;
    target.spell_hp_aura += source.spell_hp_aura;
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
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if !was_magnetized {
        tier1::wrath_weaver::after_play_minion(state, played_tribe, board_pos);
        tier4::bream_counter::after_play_minion(state, played_tribe);
        tier4::ichoron_the_protector::after_play_minion(state, played_tribe, board_pos);
        tier5::insatiable_urzul::after_play_minion(state, played_tribe, board_pos, pool, rng);
        tier5::spark_snapper::after_play_mech(state, played_tribe, board_pos, false);
        tier5::lurking_leviathan::on_beast_summoned_tavern(&mut state.board, board_pos);
    }
    tier2::mechagnome_interpreter::after_play_or_magnetize_mech(
        state,
        played_tribe,
        board_pos,
        was_magnetized,
    );
    sync_board_spell_auras(&state.board, &mut state.auras);
}

/// Apply On-Sell triggers when `sold` is sold from `board`.
pub fn on_sell(state: &mut TavernState, sold: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    sync_board_spell_auras(&state.board, &mut state.auras);
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
        tier4::air_baller::ID => tier4::air_baller::on_sell(state, sold),
        tier4::faceless_operative::ID => {
            tier4::faceless_operative::on_sell(state, sold, pool, rng)
        }
        tier4::snarky_shark::ID => tier4::snarky_shark::on_sell(state, sold, pool, rng),
        tier4::tortollan_blue_shell::ID => tier4::tortollan_blue_shell::on_sell(state, sold),
        _ => {}
    }
}

/// Apply Start-of-Turn triggers on a minion on `board` (`Patient Scout`, `Ichoron`, `Malchezaar`, `Thorned Trailblazer`).
pub fn on_start_turn(unit: &mut Unit) {
    if unit.card_id == tier2::patient_scout::ID {
        tier2::patient_scout::on_start_turn(unit);
    }
    if unit.temp_divine_shield {
        unit.divine_shield = false;
        unit.temp_divine_shield = false;
    }
    init_unit_turn_charges(unit);
}

/// Apply board-wide Start-of-Turn triggers (`Accord-o-Tron`).
pub fn on_start_turn_board(state: &mut TavernState) {
    tier3::accord_o_tron::on_start_turn(state);
}

/// Apply End-of-Turn triggers across `board` and `hand` when `EndTurn` is taken (`Drakkari Enchanter` multiplies triggers).
pub fn on_end_turn(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let repeats = tier5::drakkari_enchanter::end_of_turn_multiplier(&state.board);
    for _ in 0..repeats {
        tier1::lullabot::on_end_turn(state);
        tier2::surfing_sylvar::on_end_turn(state);
        tier3::gem_rat::on_end_turn(state);
        resolve_roogug_procs(&mut state.board, &state.auras, rng);
        tier3::trench_fighter::on_end_turn(state);
        tier4::flaming_enforcer::on_end_turn(state, pool);
        tier4::gearfin::on_end_turn(state, rng);
        tier4::nightmare_corroder::on_end_turn(state);
        tier4::parasitic_fleshling::on_end_turn(state);
        tier5::cataclysmic_harbinger::on_end_turn(state);
        tier5::felfire_conjurer::on_end_turn(state);
        tier5::mysterious_kthir::on_end_turn(state, pool, rng);
        tier5::resourceful_robot::on_end_turn(state, pool, rng);
    }
}

/// Apply board-wide observers when a Tavern spell is cast (`Timecap'n Hooktail`, `Vicious Mindslasher`, `Charging Czarina`, `Living Azerite`).
pub fn on_cast_tavern_spell(state: &mut TavernState) {
    tier3::timecapn_hooktail::on_cast_tavern_spell(state);
    tier3::vicious_mindslasher::on_cast_tavern_spell(state);
    tier5::charging_czarina::on_cast_tavern_spell(state);
    tier5::living_azerite::on_cast_tavern_spell(state);
}

/// Apply board-wide observers when a targeted spell is cast on `board[target_pos]` (`Glambot`, `Twilight Tidehunter`, `Devilish Distractor`, `Shamanic Tidecaller`).
pub fn after_cast_targeted_spell(state: &mut TavernState, target_pos: usize) {
    tier4::glambot::after_cast_targeted_spell(state, target_pos);
    tier4::twilight_tidehunter::after_cast_targeted_spell(state, target_pos);
    tier5::devilish_distractor::after_cast_targeted_spell(state, target_pos);
    tier5::shamanic_tidecaller::after_cast_targeted_spell(state, target_pos);
}

/// Apply board-wide observers after any spell is cast (`Felboar`).
pub fn after_cast_any_spell(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    tier5::felboar::after_cast_any_spell(state, pool, rng);
}

/// Return the cast multiplier for Bounty spells (`Proud Privateer`).
pub fn bounty_cast_multiplier(board: &[Unit]) -> u32 {
    tier5::proud_privateer::bounty_cast_multiplier(board)
}

/// Return the number of extra Deathrattle triggers from `Titus Rivendare` on `board`.
pub fn extra_deathrattle_triggers(board: &[Unit]) -> u32 {
    tier5::titus_rivendare::extra_deathrattle_triggers(board)
}

/// Apply board-wide observers when Gold is spent (`Gunpowder Courier`, `Air Revenant`, `Enterprising Escapee`).
pub fn on_gold_spent(state: &mut TavernState, amount: u32, pool: &mut CardPool, rng: &mut Rng) {
    tier4::gunpowder_courier::on_gold_spent(state, amount);
    tier5::air_revenant::on_gold_spent(state, amount, pool, rng);
    tier5::enterprising_escapee::on_gold_spent(state, amount, rng);
}

/// Return the number of extra times a Blood Gem played from hand should cast (`Hot-Air Surveyor`).
pub fn extra_hand_blood_gem_casts(board: &[Unit]) -> u32 {
    tier4::hot_air_surveyor::extra_hand_blood_gem_casts(board)
}

/// Hook called whenever `count` Blood Gems are played on `unit` (`Geomagus Roogug`).
pub fn on_blood_gems_played_on_unit(unit: &mut Unit, count: u32) {
    tier4::geomagus_roogug::on_blood_gems_played(unit, count);
}

/// Distribute any queued `Geomagus Roogug` Blood Gem procs to another friendly minion on `board`.
pub fn resolve_roogug_procs(board: &mut [Unit], auras: &PlayerAuras, rng: &mut Rng) {
    tier4::geomagus_roogug::resolve_procs(board, auras, rng);
}

/// Resolve discard triggers when `discarded` is discarded from hand (`Sludge Corrosion`, `Corrupted Coin`, `Energizing Chamber`, `Cutthroat K'Thir`, `Mindbender Ghur'sha`).
pub fn on_discard_hand_card(
    state: &mut TavernState,
    discarded: &Unit,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    state.auras.cards_discarded += 1;
    tier4::cutthroat_kthir::on_discard(state);
    tier5::mindbender_ghursha::on_discard(state);
    if discarded.card_id == tokens::SPELL_SLUDGE_CORROSION {
        for _ in 0..2 {
            state.auras.spells_played += 1;
            spells::cast_spell(state, tokens::make_sludge_corrosion(), 0, pool, rng);
        }
    } else if discarded.card_id == spells::SPELL_CORRUPTED_COIN {
        state.auras.base_max_gold_bonus += 2;
        state.max_gold += 2;
    } else if discarded.card_id == spells::SPELL_ENERGIZING_CHAMBER {
        if let Some(chamber) = spells::spell_by_id(spells::SPELL_ENERGIZING_CHAMBER) {
            for _ in 0..2 {
                state.auras.spells_played += 1;
                spells::cast_spell(state, chamber.clone(), 0, pool, rng);
            }
        }
    }
}

/// Returns `true` if any minion on `board` rewinds hero damage (`Soul Rewinder`, `Ashen Corruptor`).
pub fn board_prevents_hero_damage(board: &[Unit]) -> bool {
    board.iter().any(|u| {
        matches!(
            u.card_id,
            tier2::soul_rewinder::ID | tier4::ashen_corruptor::ID
        )
    })
}

/// Trigger hero-damage observers (`Soul Rewinder`, `Ashen Corruptor`, `Tichondrius`), returning `true` if the damage was rewound.
pub fn on_hero_damage_taken(board: &mut [Unit], shop: &mut [Unit]) -> bool {
    let r1 = tier2::soul_rewinder::on_hero_damage_taken(board);
    let r2 = tier4::ashen_corruptor::on_hero_damage_taken(board, shop);
    tier5::tichondrius::on_hero_damage_taken(board);
    r1 || r2
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
        tier4::dead_bellringer::ID => Some(tier4::dead_bellringer::ACTIVATE_COST),
        tier4::drone_duplicator::ID => Some(tier4::drone_duplicator::ACTIVATE_COST),
        tier4::kelp_keeper::ID => Some(tier4::kelp_keeper::ACTIVATE_COST),
        tier4::living_prison::ID => Some(tier4::living_prison::ACTIVATE_COST),
        tier4::mindbending_recruiter::ID => Some(tier4::mindbending_recruiter::ACTIVATE_COST),
        tier4::sacrificial_wrathguard::ID => Some(tier4::sacrificial_wrathguard::ACTIVATE_COST),
        tier4::sky_hatch_runaway::ID => Some(tier4::sky_hatch_runaway::ACTIVATE_COST),
        tier4::soulkeeping_jailer::ID => Some(tier4::soulkeeping_jailer::ACTIVATE_COST),
        tier5::deft_deserter::ID => Some(tier5::deft_deserter::ACTIVATE_COST),
        tier5::nraqi_frostcaller::ID => Some(tier5::nraqi_frostcaller::ACTIVATE_COST),
        tier5::sewer_escapee::ID => Some(tier5::sewer_escapee::ACTIVATE_COST),
        _ => None,
    }
}

/// Returns the target domain required by a minion's `Activate` ability.
pub fn activate_target_kind(card_id: CardId) -> ActivateTargetKind {
    match card_id {
        tier1::suspicious_prisonguard::ID => ActivateTargetKind::BoardOther,
        tier4::dead_bellringer::ID => ActivateTargetKind::BoardOtherUndead,
        tier5::sewer_escapee::ID => ActivateTargetKind::BoardOtherMurloc,
        tier4::kelp_keeper::ID | tier4::sky_hatch_runaway::ID => ActivateTargetKind::BoardAny,
        tier2::brain_rotter::ID
        | tier3::abyssal_envoy::ID
        | tier3::mangled_bandit::ID
        | tier4::mindbending_recruiter::ID
        | tier5::nraqi_frostcaller::ID => ActivateTargetKind::HandCard,
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
            tier2::lurking_lionfish::on_activate(state, source_pos, target_pos, pool, rng)
        }
        tier3::abyssal_envoy::ID => {
            tier3::abyssal_envoy::on_activate(state, source_pos, target_pos, pool, rng)
        }
        tier3::fruit_vendor::ID => tier3::fruit_vendor::on_activate(state, source_pos),
        tier3::hired_mount::ID => tier3::hired_mount::on_activate(state, source_pos, rng),
        tier3::mangled_bandit::ID => {
            tier3::mangled_bandit::on_activate(state, source_pos, target_pos, pool, rng)
        }
        tier4::dead_bellringer::ID => {
            tier4::dead_bellringer::on_activate(state, source_pos, target_pos, pool, rng)
        }
        tier4::drone_duplicator::ID => tier4::drone_duplicator::on_activate(state, source_pos),
        tier4::kelp_keeper::ID => {
            tier4::kelp_keeper::on_activate(state, source_pos, target_pos, pool, rng)
        }
        tier4::living_prison::ID => tier4::living_prison::on_activate(state, source_pos),
        tier4::mindbending_recruiter::ID => {
            tier4::mindbending_recruiter::on_activate(state, source_pos, target_pos, pool, rng)
        }
        tier4::sacrificial_wrathguard::ID => {
            tier4::sacrificial_wrathguard::on_activate(state, source_pos)
        }
        tier4::sky_hatch_runaway::ID => {
            tier4::sky_hatch_runaway::on_activate(state, source_pos, target_pos, rng)
        }
        tier4::soulkeeping_jailer::ID => {
            tier4::soulkeeping_jailer::on_activate(state, source_pos, pool, rng)
        }
        tier5::deft_deserter::ID => tier5::deft_deserter::on_activate(state, source_pos, rng),
        tier5::nraqi_frostcaller::ID => {
            tier5::nraqi_frostcaller::on_activate(state, source_pos, target_pos, pool, rng)
        }
        tier5::sewer_escapee::ID => {
            tier5::sewer_escapee::on_activate(state, source_pos, target_pos, rng)
        }
        _ => {}
    }
}

/// Apply Start-of-Combat hand summons (`Flighty Scout`).
pub fn on_start_of_combat_hand(hand: &[Unit], board: &mut Vec<Unit>) {
    tier1::flighty_scout::apply_start_of_combat_hand(hand, board);
}

/// Summon `Boon of Beetles` Taunt Beetles into any open board slots during combat.
pub fn summon_boon_of_beetles(
    side: Side,
    board: &mut Vec<Unit>,
    auras: &mut PlayerAuras,
    combat_beast_bonus_atk: i32,
    next_id: &mut UnitId,
    events: &mut Vec<Event>,
) {
    while auras.boon_of_beetles_charges > 0 && board.len() < MAX_BOARD_SIZE {
        auras.boon_of_beetles_charges -= 1;
        let mut beetle = tokens::make_beetle(false, auras).with_keyword(Keyword::Taunt);
        beetle.id = *next_id;
        *next_id += 1;
        apply_combat_summon_modifiers(board, auras, combat_beast_bonus_atk, beetle.id, &mut beetle);
        events.push(Event::UnitSummoned {
            side,
            source: beetle.id,
            unit: beetle.id,
            name: beetle.name.clone(),
            attack: beetle.attack,
            health: beetle.health,
            reason: "Boon of Beetles",
        });
        board.push(beetle);
    }
}

/// Resolve Start-of-Combat board triggers for one side (`Flighty Scout`, `Electric Synthesizer`, `Humming Bird`, `Amber Guardian`, `Diremuck Forager`, `Runic Arcanist`, `Costume Enthusiast`, `Hopebringer`, `Boon of Beetles`).
#[allow(clippy::too_many_arguments)]
pub fn on_start_of_combat(
    side: Side,
    board: &mut Vec<Unit>,
    auras: &mut PlayerAuras,
    hand: &[Unit],
    hand_summoned: &mut [bool],
    combat_beast_bonus_atk: &mut i32,
    next_id: &mut UnitId,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    tier1::flighty_scout::on_start_of_combat(
        side,
        hand,
        board,
        auras,
        *combat_beast_bonus_atk,
        next_id,
        events,
    );
    summon_boon_of_beetles(side, board, auras, *combat_beast_bonus_atk, next_id, events);
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
            tier4::runic_arcanist::ID => {
                tier4::runic_arcanist::on_start_of_combat(side, board, auras, is_golden, events);
            }
            tier5::costume_enthusiast::ID => {
                tier5::costume_enthusiast::on_start_of_combat(
                    side, board, id, is_golden, hand, events,
                );
            }
            tier5::hopebringer::ID => {
                tier5::hopebringer::on_start_of_combat(side, board, id, is_golden, events);
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
    target: Option<&mut Unit>,
    auras: &mut PlayerAuras,
    hand: &[Unit],
    hand_summoned: &mut [bool],
    generated_hand: &mut Vec<Unit>,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) -> Vec<Unit> {
    let card_id = board[attacker_pos].card_id;
    let attacker_id = board[attacker_pos].id;
    let is_golden = board[attacker_pos].is_golden;
    let summons = match card_id {
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
        tier4::bigwig_bandit::ID => {
            tier4::bigwig_bandit::on_rally(&board[attacker_pos], generated_hand, rng);
            Vec::new()
        }
        tier4::bonker::ID => {
            tier4::bonker::on_rally(side, board, attacker_pos, auras, events);
            Vec::new()
        }
        tier4::bramble_tunneler::ID => {
            tier4::bramble_tunneler::on_rally(&board[attacker_pos], generated_hand, rng);
            Vec::new()
        }
        tier4::bronze_timewalker::ID => {
            tier4::bronze_timewalker::on_rally(&board[attacker_pos], generated_hand, rng);
            Vec::new()
        }
        tier4::dark_paradox::ID => {
            tier4::dark_paradox::on_rally(board, attacker_pos, auras, generated_hand, rng);
            Vec::new()
        }
        tier4::headhunter_gryphon::ID => {
            tier4::headhunter_gryphon::on_rally(
                &board[attacker_pos],
                auras,
                generated_hand,
                rng,
            );
            Vec::new()
        }
        tier4::heroic_underdog::ID => {
            tier4::heroic_underdog::on_rally(&mut board[attacker_pos], target.as_deref());
            Vec::new()
        }
        tier4::hoarding_hyena::ID => tier4::hoarding_hyena::on_rally(&board[attacker_pos]),
        tier4::sindorei_straight_shot::ID => {
            tier4::sindorei_straight_shot::on_rally(target);
            Vec::new()
        }
        tier5::bile_spitter::ID => {
            tier5::bile_spitter::on_rally(board, attacker_pos, rng);
            Vec::new()
        }
        tier5::razorfen_vineweaver::ID => {
            tier5::razorfen_vineweaver::on_rally(&mut board[attacker_pos], auras, true);
            Vec::new()
        }
        tier5::sanguine_refiner::ID => {
            tier5::sanguine_refiner::on_rally(&board[attacker_pos], auras);
            Vec::new()
        }
        _ => Vec::new(),
    };
    resolve_roogug_procs(board, auras, rng);
    summons
}

/// Trigger a friendly minion's `Rally` effect on `state.board[target_pos]` during the Tavern Phase (`Sky-hatch Runaway`).
pub fn trigger_tavern_rally(state: &mut TavernState, target_pos: usize, rng: &mut Rng) {
    if target_pos >= state.board.len() {
        return;
    }
    let mut generated_hand = Vec::new();
    let mut hand_summoned = vec![false; state.hand.len()];
    let mut events = Vec::new();
    let summons = on_rally(
        Side::A,
        &mut state.board,
        target_pos,
        None,
        &mut state.auras,
        &state.hand,
        &mut hand_summoned,
        &mut generated_hand,
        rng,
        &mut events,
    );
    for card in generated_hand {
        state.add_to_hand(card);
    }
    let mut insert_pos = (target_pos + 1).min(state.board.len());
    for mut token in summons {
        if state.board.len() < MAX_BOARD_SIZE {
            state.apply_global_unit_auras(&mut token);
            let cid = token.card_id;
            state.board.insert(insert_pos, token);
            tier5::lurking_leviathan::on_beast_summoned_tavern(&mut state.board, insert_pos);
            insert_pos += 1;
            state.check_and_resolve_triple(cid);
        }
    }
    state.sync_all_auras();
}

/// Resolve board-wide observers when a friendly minion attacks (`Prodigious Tusker`, `Roaring Recruiter`, `Cage Gnawer`).
pub fn on_friendly_attack(
    side: Side,
    board: &mut [Unit],
    attacker_id: UnitId,
    auras: &PlayerAuras,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    tier2::prodigious_tusker::on_friendly_attack(side, board, attacker_id, auras, events);
    tier3::roaring_recruiter::on_friendly_attack(side, board, attacker_id, events);
    tier4::cage_gnawer::on_friendly_attack(side, board, attacker_id, events);
    resolve_roogug_procs(board, auras, rng);
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

/// Returns `true` if `card_id` also damages adjacent enemies when attacking (`Blade Collector`).
pub fn cleaves_adjacent_enemies(card_id: CardId) -> bool {
    card_id == tier4::blade_collector::ID
}

/// Returns `true` if `card_id` deals excess attack damage to adjacent enemy(ies) (`Wildfire Elemental`).
pub fn deals_excess_damage_to_neighbors(card_id: CardId) -> bool {
    card_id == tier3::wildfire_elemental::ID
}

/// Resolve on-damage-taken triggers (`Very Hungry Winterfinner`).
pub fn on_damage_taken(unit: &Unit, hand: &mut [Unit], rng: &mut Rng) {
    tier2::very_hungry_winterfinner::on_damage_taken(unit, hand, rng);
}

/// Update persistent player aura counters and basic Avenge triggers when a friendly minion dies (`Eternal Knight`, `Relentless Deflector`).
pub fn on_unit_died(unit: &Unit, surviving_board: &mut [Unit], auras: &mut PlayerAuras) {
    if unit.card_id == tier2::eternal_knight::ID {
        auras.eternal_knights_died += 1;
    }
    for survivor in surviving_board.iter_mut() {
        tier3::relentless_deflector::on_friendly_death(survivor);
    }
}

/// Resolve all friendly-death observers and `Avenge` triggers during combat (`Eternal Knight`, `Relentless Deflector`, `Drustfallen Butcher`, `Lichling Hoarder`, `Eternal Tycoon`).
#[allow(clippy::too_many_arguments)]
pub fn on_combat_friendly_death(
    side: Side,
    dying: &Unit,
    surviving_board: &mut Vec<Unit>,
    auras: &mut PlayerAuras,
    hand: &mut Vec<Unit>,
    hand_summoned: &mut Vec<bool>,
    combat_beast_bonus_atk: i32,
    next_id: &mut UnitId,
    pending_immediate_attacks: &mut Vec<UnitId>,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    on_unit_died(dying, surviving_board, auras);
    for survivor in surviving_board.iter_mut() {
        tier5::drustfallen_butcher::on_friendly_death(survivor, hand, hand_summoned, auras);
    }
    let mut snapshot: Vec<Unit> = surviving_board.clone();
    snapshot.push(dying.clone());
    for survivor in surviving_board.iter_mut() {
        tier5::lichling_hoarder::on_friendly_death(
            survivor,
            &snapshot,
            hand,
            hand_summoned,
            auras,
            rng,
        );
    }
    tier5::eternal_tycoon::on_friendly_death(
        side,
        surviving_board,
        auras,
        combat_beast_bonus_atk,
        next_id,
        events,
        pending_immediate_attacks,
    );
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
        tier4::conveyor_construct::ID => tier4::conveyor_construct::on_deathrattle(dying, ctx),
        tier4::friendly_geist::ID => tier4::friendly_geist::on_deathrattle(dying, ctx),
        tier4::gormling_gourmet::ID => tier4::gormling_gourmet::on_deathrattle(dying, ctx),
        tier4::imp_lusionist::ID => tier4::imp_lusionist::on_deathrattle(dying, ctx),
        tier4::leyline_surfacer::ID => tier4::leyline_surfacer::on_deathrattle(dying, ctx),
        tier4::plaguerunner::ID => tier4::plaguerunner::on_deathrattle(dying, ctx),
        tier4::razorfen_flapper::ID => tier4::razorfen_flapper::on_deathrattle(dying, ctx),
        tier4::sacrificial_wrathguard::ID => {
            tier4::sacrificial_wrathguard::on_deathrattle(dying, ctx)
        }
        tier5::draconic_warden::ID => tier5::draconic_warden::on_deathrattle(dying, ctx),
        tier5::eternal_summoner::ID => tier5::eternal_summoner::on_deathrattle(dying, ctx),
        tier5::faceless_converter::ID => tier5::faceless_converter::on_deathrattle(dying, ctx),
        tier5::firescale_hoarder::ID => tier5::firescale_hoarder::on_deathrattle(dying, ctx),
        tier5::ghastcoiler::ID => tier5::ghastcoiler::on_deathrattle(dying, ctx),
        tier5::goldrinn_the_great_wolf::ID => {
            tier5::goldrinn_the_great_wolf::on_deathrattle(dying, ctx)
        }
        tier5::nightmare_par_tea_guest::ID => {
            tier5::nightmare_par_tea_guest::on_deathrattle(dying, ctx)
        }
        tier5::nraqi_sapper::ID => tier5::nraqi_sapper::on_deathrattle(dying, ctx),
        tier5::sewer_lord::ID => tier5::sewer_lord::on_deathrattle(dying, ctx),
        tier5::ship_master_eudora::ID => tier5::ship_master_eudora::on_deathrattle(dying, ctx),
        tier5::shipwrecked_rascal::ID => tier5::shipwrecked_rascal::on_deathrattle(dying, ctx),
        tier5::turquoise_skitterer::ID => tier5::turquoise_skitterer::on_deathrattle(dying, ctx),
        tokens::TOKEN_SEWER_RAT => tier5::sewer_lord::on_sewer_rat_deathrattle(dying, ctx),
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

/// Apply post-combat persistence from a combat unit back to its Tavern counterpart (`Tarecgosa`, `Persistent Poet`, `Devout Hellcaller`, `Razorfen Vineweaver`, `Ship Master Eudora`, `Hopebringer`, `Lurking Leviathan`, `Treasure Parrot`).
pub fn on_post_combat_unit(
    pre_board: &[Unit],
    idx: usize,
    post_combat_board: &[Unit],
    tavern_unit: &mut Unit,
) {
    let Some(pre_combat) = pre_board.get(idx) else {
        return;
    };
    if tavern_unit.card_id == tier2::tarecgosa::ID {
        tier2::tarecgosa::apply_post_combat_persistence(
            pre_combat,
            post_combat_board,
            tavern_unit,
        );
    }
    tier4::persistent_poet::on_post_combat_adjacent_dragon(
        pre_board,
        idx,
        post_combat_board,
        tavern_unit,
    );
    if let Some(post) = post_combat_board.iter().find(|u| u.id == pre_combat.id) {
        if post.perm_atk_gained != 0 || post.perm_hp_gained != 0 {
            tavern_unit.add_stats(post.perm_atk_gained, post.perm_hp_gained);
        }
        if post.perm_blood_gems_gained > 0 {
            tavern_unit.blood_gems_played += post.perm_blood_gems_gained;
            tavern_unit.blood_gem_stats_applied.0 += post.perm_atk_gained;
            tavern_unit.blood_gem_stats_applied.1 += post.perm_hp_gained;
        }
        tavern_unit.hopebringer_stacks = post.hopebringer_stacks;
        tavern_unit.leviathan_stacks = post.leviathan_stacks;
        if tavern_unit.card_id == tier3::treasure_parrot::ID {
            tavern_unit.damage_dealt_counter = post.damage_dealt_counter;
            tavern_unit.threshold_triggered = post.threshold_triggered;
        }
    }
}
