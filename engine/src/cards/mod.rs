//! Card templates, catalogs, and generic per-card hook dispatch.
//!
//! Card definitions are organized by folder (`src/cards/tier1/` .. `src/cards/tier7/`,
//! `src/cards/deities.rs`, `src/cards/spells.rs`, `src/cards/tokens.rs`). Each card declares
//! its behaviour as a [`CardHooks`] table (see [`hooks`](mod@hooks)); the dispatch functions
//! in this module look hooks up by `CardId` through the [registry](fn@hooks) and never name a
//! specific card.

pub mod deities;
pub mod hooks;
mod registry;
pub mod spells;
pub mod tier1;
pub mod tier2;
pub mod tier3;
pub mod tier4;
pub mod tier5;
pub mod tier6;
pub mod tier7;
pub mod tokens;

pub use hooks::{CardFlags, CardHooks, Passive, Played, SocAction};
pub use registry::{all_templates, hooks, template};

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
    /// Requires any friendly minion on `board`.
    BoardAny,
    /// Requires any friendly Battlecry minion on `board` (`Kelp Keeper`).
    BoardBattlecry,
    /// Requires any friendly Rally minion on `board` (`Sky-hatch Runaway`).
    BoardRally,
    /// Requires a card in `hand` (`Brain Rotter`, `Abyssal Envoy`, `Mangled Bandit`, `Mindbending Recruiter`, `N'raqi Frostcaller`).
    HandCard,
    /// Requires a card in `shop` (`Lurking Lionfish`).
    ShopCard,
}

/// One side's board plus everything a combat-style hook may touch (Deathrattles,
/// Start-of-Combat triggers, death/attack/summon observers).
///
/// Supports summoning tokens at the current board slot (`cursor`), buffing friendly
/// minions, mutating persistent `auras`, and adding/modifying cards in `hand`. Also used
/// in the Tavern (`in_combat = false`) when a minion is destroyed there.
pub struct BoardCtx<'a> {
    pub side: Side,
    pub in_combat: bool,
    pub board: &'a mut Vec<Unit>,
    /// Board slot where the next summon is inserted.
    pub cursor: usize,
    pub auras: &'a mut PlayerAuras,
    pub hand: &'a mut Vec<Unit>,
    pub hand_summoned: &'a mut Vec<bool>,
    pub dead_aberrations: &'a [Unit],
    /// Temporary combat Attack bonus for friendly Beasts.
    pub beast_bonus_atk: &'a mut i32,
    pub hero_tier: u32,
    /// Friendly minions queued to attack immediately.
    pub pending_attacks: &'a mut Vec<UnitId>,
    /// Enemy minions this effect destroys (resolved by the engine after the hook returns).
    pub enemy_destroys: Vec<UnitId>,
    pub next_id: &'a mut UnitId,
    pub rng: &'a mut Rng,
    pub events: &'a mut Vec<Event>,
}

/// Former name of [`BoardCtx`].
pub type DeathrattleContext<'a> = BoardCtx<'a>;

impl<'a> BoardCtx<'a> {
    /// Summon `token` at the current board position (`self.cursor`) if the board has space
    /// (`< MAX_BOARD_SIZE`), applying all active summon modifiers.
    pub fn summon(&mut self, source_id: UnitId, token: Unit) {
        self.summon_as(source_id, token, "Deathrattle");
    }

    /// [`summon`](Self::summon), logging the summon with `reason`. Returns the summoned unit's
    /// board slot, or `None` if the board was full.
    pub fn summon_as(
        &mut self,
        source_id: UnitId,
        mut token: Unit,
        reason: &'static str,
    ) -> Option<usize> {
        if self.board.len() >= MAX_BOARD_SIZE {
            return None;
        }
        token.id = *self.next_id;
        *self.next_id += 1;
        if self.in_combat {
            apply_combat_summon_modifiers(
                self.board,
                self.auras,
                *self.beast_bonus_atk,
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
            reason,
        });
        let pos = self.cursor.min(self.board.len());
        self.board.insert(pos, token);
        self.cursor = pos + 1;
        if !self.in_combat {
            on_tavern_summon(self.board, pos);
        }
        Some(pos)
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
            on_card_added_to_hand(self.board, self.auras);
        }
    }

    /// Request that the enemy minion `unit` be destroyed once this hook returns.
    pub fn destroy_enemy(&mut self, unit: UnitId) {
        self.enemy_destroys.push(unit);
    }
}

/// Everything a `Rally` (On-Attack) hook may touch.
pub struct RallyCtx<'a> {
    pub side: Side,
    /// `false` when the Rally is triggered in the Tavern (`Sky-hatch Runaway`).
    pub in_combat: bool,
    pub board: &'a mut [Unit],
    pub attacker_pos: usize,
    pub attacker_id: UnitId,
    pub is_golden: bool,
    /// The defending board and the attack target's index (combat only).
    pub def_target: Option<(&'a mut [Unit], usize)>,
    pub auras: &'a mut PlayerAuras,
    pub hand: &'a [Unit],
    pub hand_summoned: &'a mut [bool],
    /// Cards to add to hand once the Rally resolves.
    pub generated_hand: &'a mut Vec<Unit>,
    /// Set by the hook if the returned summons should strike the attack target first.
    pub summons_attack_target: bool,
    pub rng: &'a mut Rng,
    pub events: &'a mut Vec<Event>,
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
    /// The card's behaviour (see [`CardHooks`]).
    pub hooks: CardHooks,
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
            hooks: CardHooks::EMPTY,
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
        self.hooks.activate_cost = Some(cost);
        self
    }

    pub fn with_activate_target(mut self, target: ActivateTargetKind) -> Self {
        self.hooks.activate_target = target;
        self
    }

    pub fn with_flags(mut self, flags: CardFlags) -> Self {
        self.hooks.flags = self.hooks.flags | flags;
        self
    }

    /// Board-aura bonus to Tavern spell stats while this is on board (doubled when Golden).
    pub fn with_spell_aura(mut self, atk: i32, hp: i32) -> Self {
        self.hooks.spell_aura = (atk, hp);
        self
    }

    /// Gold cost of this card's `Activate` ability, if it has one.
    pub fn activate_cost(&self) -> Option<u32> {
        self.hooks.activate_cost
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
        if let Some(reset_turn_charges) = self.hooks.reset_turn_charges {
            reset_turn_charges(&mut u);
        }
        set_spell_aura(&mut u, self.hooks.spell_aura);
        u
    }
}

/// Initialize or reset per-turn charges on `unit` (the card's `reset_turn_charges` hook).
pub fn init_unit_turn_charges(unit: &mut Unit) {
    if let Some(f) = hooks(unit.card_id).reset_turn_charges {
        f(unit);
    }
}

/// Set `unit`'s board-aura spell bonus from its card's `spell_aura` (doubled when Golden).
pub fn init_spell_aura(unit: &mut Unit) {
    set_spell_aura(unit, hooks(unit.card_id).spell_aura);
}

fn set_spell_aura(unit: &mut Unit, (atk, hp): (i32, i32)) {
    if atk != 0 || hp != 0 {
        let m = if unit.is_golden { 2 } else { 1 };
        unit.spell_atk_aura = atk * m;
        unit.spell_hp_aura = hp * m;
    }
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

/// All 32 active Solo Tier 6 minions (Patch 36.6.3).
pub fn tier6_catalog() -> Vec<CardTemplate> {
    tier6::catalog()
}

/// Alias for [`tier6_catalog`].
pub fn solo_tier_6_catalog() -> Vec<CardTemplate> {
    tier6_catalog()
}

/// All 12 active Solo Tier 7 minions (Patch 36.6.3).
pub fn tier7_catalog() -> Vec<CardTemplate> {
    tier7::catalog()
}

/// Alias for [`tier7_catalog`].
pub fn solo_tier_7_catalog() -> Vec<CardTemplate> {
    tier7_catalog()
}

/// Full active catalog (Solo Tier 1..=7 = 252 minions).
pub fn full_catalog() -> Vec<CardTemplate> {
    let mut cards = tier1_catalog();
    cards.extend(tier2_catalog());
    cards.extend(tier3_catalog());
    cards.extend(tier4_catalog());
    cards.extend(tier5_catalog());
    cards.extend(tier6_catalog());
    cards.extend(tier7_catalog());
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
        "tier6" | "solo_tier_6" => Ok(tier6_catalog()),
        "tier7" | "solo_tier_7" => Ok(tier7_catalog()),
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
        "tier1_5" => {
            let mut c = tier1_catalog();
            c.extend(tier2_catalog());
            c.extend(tier3_catalog());
            c.extend(tier4_catalog());
            c.extend(tier5_catalog());
            Ok(c)
        }
        "tier1_6" => {
            let mut c = tier1_catalog();
            c.extend(tier2_catalog());
            c.extend(tier3_catalog());
            c.extend(tier4_catalog());
            c.extend(tier5_catalog());
            c.extend(tier6_catalog());
            Ok(c)
        }
        "full" | "solo_full" | "tier1_7" => Ok(full_catalog()),
        other => Err(format!(
            "unknown catalog {other:?}; expected one of: tier1, solo_tier_1, tier2, solo_tier_2, tier3, solo_tier_3, tier4, solo_tier_4, tier5, solo_tier_5, tier6, solo_tier_6, tier7, solo_tier_7, tier1_2, tier1_3, tier1_4, tier1_5, tier1_6, tier1_7, full, solo_full, test"
        )),
    }
}

/// Instantiate a plain (non-Golden, unbuffed) copy of `unit`.
pub fn instantiate_plain_copy(unit: &Unit) -> Unit {
    if let Some(tpl) = template(unit.card_id) {
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

/// Returns `true` if `card_id` has a Battlecry.
pub fn is_battlecry_minion(card_id: CardId) -> bool {
    hooks(card_id).battlecry.is_some()
}

/// Returns `true` if `card_id` has a Choose One.
pub fn is_choose_one_minion(card_id: CardId) -> bool {
    hooks(card_id).choose_one.is_some()
}

/// Returns `true` if `card_id` has a Rally.
pub fn is_rally_minion(card_id: CardId) -> bool {
    hooks(card_id).rally.is_some()
}

/// Returns `true` if `card_id` has a Deathrattle.
pub fn is_deathrattle_minion(card_id: CardId) -> bool {
    hooks(card_id).deathrattle.is_some()
}

// ---------------------------------------------------------------------------
// Unified Tier-Agnostic Tavern & Combat Hook Dispatch
// ---------------------------------------------------------------------------

/// Board units carrying the hook selected by `get`, left to right: `(board_idx, card_id, hook)`.
fn observers<F>(board: &[Unit], get: impl Fn(&CardHooks) -> Option<F>) -> Vec<(usize, CardId, F)> {
    board
        .iter()
        .enumerate()
        .filter_map(|(idx, u)| get(hooks(u.card_id)).map(|f| (idx, u.card_id, f)))
        .collect()
}

/// Invoke the Tavern observer hook selected by `get` once for every friendly board unit that
/// carries it, left to right. Observers are snapshotted up front; an observer whose slot no
/// longer holds the same card when its turn comes (the board changed underneath it) is skipped.
fn notify_tavern<F>(
    state: &mut TavernState,
    get: impl Fn(&CardHooks) -> Option<F>,
    mut call: impl FnMut(&mut TavernState, usize, F),
) {
    for (idx, card_id, f) in observers(&state.board, get) {
        if state.board.get(idx).is_some_and(|u| u.card_id == card_id) {
            call(state, idx, f);
        }
    }
}

/// Invoke the observer hook selected by `get` for every friendly unit on `ctx.board` carrying
/// it, left to right. Observers are snapshotted up front and re-located before each call: by unit
/// id in combat (where only living units observe), by slot in the Tavern.
fn notify_board<'a, F>(
    ctx: &mut BoardCtx<'a>,
    get: impl Fn(&CardHooks) -> Option<F>,
    mut call: impl FnMut(&mut BoardCtx<'a>, usize, F),
) {
    let snapshot: Vec<(usize, UnitId, CardId, F)> = ctx
        .board
        .iter()
        .enumerate()
        .filter_map(|(idx, u)| get(hooks(u.card_id)).map(|f| (idx, u.id, u.card_id, f)))
        .collect();
    for (idx, id, card_id, f) in snapshot {
        let pos = if ctx.in_combat {
            ctx.board.iter().position(|u| u.id == id && u.health > 0)
        } else {
            ctx.board
                .get(idx)
                .filter(|u| u.card_id == card_id)
                .map(|_| idx)
        };
        if let Some(pos) = pos {
            call(ctx, pos, f);
        }
    }
}

/// Combined value of `passive` across `board`: the largest contribution (at least 1) for
/// multipliers, the sum of contributions otherwise (see [`Passive`]).
pub fn board_passive(board: &[Unit], passive: Passive) -> u32 {
    let values = board
        .iter()
        .map(|u| hooks(u.card_id).passive_of(u, passive));
    if passive.is_multiplier() {
        values.fold(1, u32::max)
    } else {
        values.sum()
    }
}

/// Run stat-threshold checks whenever a unit's stats change (in Tavern or Combat).
pub fn check_stat_thresholds(unit: &mut Unit) {
    if unit.card_id == tier1::scarlet_survivor::ID {
        tier1::scarlet_survivor::check_threshold(unit);
    }
}

/// Synchronize a unit's persistent "wherever this is" auras (`Undead` attack, `Eternal Knight`, `Volumizers`, `Relentless Deflector`, `Holy Vanguard`, `Maritime Extortionist`, `Falling Sky Golem`).
pub fn sync_unit_auras(unit: &mut Unit, auras: &PlayerAuras) {
    if unit.is_spell {
        return;
    }
    tier2::nerubian_deathswarmer::sync_unit_undead_attack(unit, auras);
    tier2::eternal_knight::sync_unit(unit, auras);
    tier3::relentless_deflector::sync_taunt(unit);
    tier4::holy_vanguard::sync_unit(unit, auras);
    tier4::maritime_extortionist::sync_unit(unit, auras);
    if unit.spell_atk_aura == 0 && unit.spell_hp_aura == 0 {
        init_spell_aura(unit);
    }
    tier6::falling_sky_golem::sync_aura(unit, auras);
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

/// Construct the Reborn resummon copy of `dying` (base or Golden-base copy with `health = 1`, `reborn = false`,
/// printed keywords restored, and global auras applied).
pub fn make_reborn_copy(dying: &Unit, auras: &PlayerAuras) -> Unit {
    let mut copy = if let Some(tpl) = template(dying.card_id) {
        let mut u = tpl.instantiate();
        if dying.is_golden {
            u.make_golden();
            u.intrinsic_golden = dying.intrinsic_golden;
        }
        u
    } else if let Some(mut tok) = tokens::make_plain_token(dying, &PlayerAuras::default()) {
        if dying.is_golden {
            tok.make_golden();
            tok.intrinsic_golden = dying.intrinsic_golden;
        }
        tok
    } else {
        let mut u = Unit::new(dying.name.clone(), dying.base_attack, dying.base_health)
            .with_card_id(dying.card_id)
            .with_tavern_tier(dying.tavern_tier)
            .with_tribe(dying.tribe)
            .with_golden(dying.is_golden);
        u.intrinsic_golden = dying.intrinsic_golden;
        u.taunt = dying.taunt;
        u.divine_shield = dying.inherent_divine_shield;
        u.inherent_divine_shield = dying.inherent_divine_shield;
        u.windfury = dying.windfury;
        u.venomous = dying.venomous;
        u.stealth = dying.stealth;
        u.magnetic = dying.magnetic;
        u
    };
    copy.health = 1;
    copy.reborn = false;
    sync_unit_auras(&mut copy, auras);
    copy.sync_max_stats();
    check_stat_thresholds(&mut copy);
    copy
}

/// Apply friendly-summon observers to the minion just summoned at `board[pos]` in the Tavern.
pub fn on_tavern_summon(board: &mut [Unit], pos: usize) {
    let mut summoned = board[pos].clone();
    notify_friendly_summon(board, Some(pos), &mut summoned, false);
    board[pos] = summoned;
}

/// Apply all combat summon modifiers (persistent auras, `Goldrinn`, the combat Beast bonus,
/// `friendly_summon` observers, stat thresholds) to `token` before it joins `board`.
pub fn apply_combat_summon_modifiers(
    board: &mut [Unit],
    auras: &PlayerAuras,
    combat_beast_bonus_atk: i32,
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
    notify_friendly_summon(board, None, token, true);
    token.sync_max_stats();
    check_stat_thresholds(token);
}

/// Let friendly units react to `summoned` joining `board` (`friendly_summon` hooks, left to
/// right). `skip` is the summoned unit's own slot if it is already on the board; in combat only
/// living units observe.
fn notify_friendly_summon(
    board: &mut [Unit],
    skip: Option<usize>,
    summoned: &mut Unit,
    in_combat: bool,
) {
    for (idx, unit) in board.iter_mut().enumerate() {
        if Some(idx) == skip || (in_combat && (unit.health <= 0 || unit.id == summoned.id)) {
            continue;
        }
        if let Some(friendly_summon) = hooks(unit.card_id).friendly_summon {
            friendly_summon(unit, summoned, in_combat);
        }
    }
}

/// Resummon `dying` with Reborn at `ctx.cursor` (if the board has room), then notify
/// `after_friendly_reborn` observers. Returns `true` if the copy was summoned.
pub fn reborn(ctx: &mut BoardCtx<'_>, dying: &Unit) -> bool {
    let copy = make_reborn_copy(dying, ctx.auras);
    let Some(pos) = ctx.summon_as(dying.id, copy, "Reborn") else {
        return false;
    };
    let reborn_attack = ctx.board[pos].attack;
    notify_board(
        ctx,
        |h| h.after_friendly_reborn,
        |c, idx, f| f(c, idx, reborn_attack),
    );
    true
}

/// Apply on-play Battlecry / Choose-One when `unit` is played from `hand` onto `board` at
/// `board_pos`. Battlecries repeat per [`Passive::BattlecryTriggers`], each followed by the
/// `after_friendly_battlecry` observers.
pub fn on_play_battlecry(
    state: &mut TavernState,
    unit: &mut Unit,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let card = hooks(unit.card_id);
    if let Some(battlecry) = card.battlecry {
        let repeats = board_passive(&state.board, Passive::BattlecryTriggers);
        for _ in 0..repeats {
            battlecry(state, unit, board_pos, pool, rng);
            notify_tavern(
                state,
                |h| h.after_friendly_battlecry,
                |s, idx, f| f(s, idx, Some(&mut *unit)),
            );
        }
    } else if let Some(choose_one) = card.choose_one {
        choose_one(state, unit, board_pos, pool, rng);
    }
}

/// Trigger the Battlecry of an existing minion at `state.board[board_pos]` (`Young Murk-Eye`, `Kelp Keeper`).
pub fn trigger_board_battlecry(
    state: &mut TavernState,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let Some(battlecry) = state
        .board
        .get(board_pos)
        .and_then(|u| hooks(u.card_id).battlecry)
    else {
        return;
    };
    let repeats = board_passive(&state.board, Passive::BattlecryTriggers);
    let mut unit = state.board[board_pos].clone();
    let old_atk = unit.attack;
    let old_hp = unit.health;
    for _ in 0..repeats {
        battlecry(state, &mut unit, board_pos, pool, rng);
        notify_tavern(
            state,
            |h| h.after_friendly_battlecry,
            |s, idx, f| f(s, idx, None),
        );
    }
    if board_pos < state.board.len() && state.board[board_pos].card_id == unit.card_id {
        let d_atk = unit.attack - old_atk;
        let d_hp = unit.health - old_hp;
        if d_atk != 0 || d_hp != 0 {
            state.board[board_pos].add_stats(d_atk, d_hp);
        }
        state.board[board_pos].taunt |= unit.taunt;
        state.board[board_pos].divine_shield |= unit.divine_shield;
        state.board[board_pos].windfury |= unit.windfury;
        state.board[board_pos].reborn |= unit.reborn;
        state.board[board_pos].venomous |= unit.venomous;
        state.board[board_pos].stealth |= unit.stealth;
    }
}

/// Apply first-time play or Magnetize triggers (`play_or_magnetize` hook).
pub fn on_first_play_or_magnetize(state: &mut TavernState, unit: &mut Unit) {
    if let Some(play_or_magnetize) = hooks(unit.card_id).play_or_magnetize {
        play_or_magnetize(state, unit);
    }
}

/// Transfer Magnetization state (enchantments carried by `source`, spell auras, Blood Gems,
/// Magnetization count, and the card's own `magnetize_transfer` hook) when `source` is
/// Magnetized onto `target`.
pub fn on_magnetize_transfer(source: &Unit, target: &mut Unit) {
    target.magnetizations_count += 1 + source.magnetizations_count;
    target.eot_health_bonus += source.eot_health_bonus;
    target.sot_gold_bonus += source.sot_gold_bonus;
    target.spell_atk_aura += source.spell_atk_aura;
    target.spell_hp_aura += source.spell_hp_aura;
    target.blood_gems_played += source.blood_gems_played;
    target.blood_gem_stats_applied.0 += source.blood_gem_stats_applied.0;
    target.blood_gem_stats_applied.1 += source.blood_gem_stats_applied.1;
    if let Some(transfer) = hooks(source.card_id).magnetize_transfer {
        transfer(source, target);
    }
}

/// Magnetize `card` onto `state.board[target_pos]` once: the target gains its stats, keywords,
/// and carried state, then `after_friendly_play` observers see the Magnetization.
pub fn apply_magnetization(
    state: &mut TavernState,
    card: &Unit,
    target_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let target = &mut state.board[target_pos];
    target.add_stats(card.attack, card.health);
    target.taunt |= card.taunt;
    target.divine_shield |= card.divine_shield;
    target.inherent_divine_shield |= card.inherent_divine_shield;
    target.windfury |= card.windfury;
    target.reborn |= card.reborn;
    target.venomous |= card.venomous;
    target.stealth |= card.stealth;
    target.magnetic |= card.magnetic;
    on_magnetize_transfer(card, target);
    let played = Played {
        card_id: card.card_id,
        tribe: card.tribe,
        board_pos: target_pos,
        magnetized: true,
    };
    after_play_minion(state, &played, pool, rng);
}

/// Magnetize `card` (from outside the board) onto `state.board[target_pos]`: its first
/// play-or-Magnetize trigger, then one application per Magnetization queued on the target, each
/// followed by the `after_friendly_magnetize` observers.
pub fn magnetize(
    state: &mut TavernState,
    card: &mut Unit,
    target_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    on_first_play_or_magnetize(state, card);
    let card: &Unit = card;
    let repeats = 1 + std::mem::take(&mut state.board[target_pos].extra_magnetize_this_turn);
    for _ in 0..repeats {
        apply_magnetization(state, card, target_pos, pool, rng);
        notify_tavern(
            state,
            |h| h.after_friendly_magnetize,
            |s, idx, f| f(s, idx, card, target_pos, pool, rng),
        );
    }
}

/// Apply board-wide observers after a minion is played (and thereby summoned) or Magnetized onto
/// `state.board[played.board_pos]`: summon observers (plays only), `after_friendly_play`
/// observers left to right (a played minion does not observe its own play), hand observers,
/// then a board spell-aura resync.
pub fn after_play_minion(
    state: &mut TavernState,
    played: &Played,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if !played.magnetized && played.board_pos < state.board.len() {
        on_tavern_summon(&mut state.board, played.board_pos);
    }
    notify_tavern(
        state,
        |h| h.after_friendly_play,
        |s, idx, f| {
            if played.magnetized || idx != played.board_pos {
                f(s, idx, played, pool, rng);
            }
        },
    );
    for unit in &mut state.hand {
        if let Some(after_play_in_hand) = hooks(unit.card_id).after_friendly_play_in_hand {
            after_play_in_hand(unit, played);
        }
    }
    sync_board_spell_auras(&state.board, &mut state.auras);
}

/// Apply On-Sell triggers when `sold` is sold from `board`.
pub fn on_sell(state: &mut TavernState, sold: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    sync_board_spell_auras(&state.board, &mut state.auras);
    tier6::twisted_wrathguard::after_sell_minion(state);
    if let Some(sell) = hooks(sold.card_id).sell {
        sell(state, sold, pool, rng);
    }
}

/// Apply Start-of-Turn upkeep to a minion on `board`: its `turn_start_unit` hook, expiry of
/// temporary Divine Shield, and per-turn charge resets.
pub fn on_start_turn(unit: &mut Unit) {
    if let Some(turn_start_unit) = hooks(unit.card_id).turn_start_unit {
        turn_start_unit(unit);
    }
    if unit.temp_divine_shield {
        unit.divine_shield = false;
        unit.temp_divine_shield = false;
    }
    init_unit_turn_charges(unit);
}

/// Apply board-wide Start-of-Turn triggers (after Gold is refreshed): Magnetized Gold bonuses,
/// then each minion's `turn_start` hook, left to right.
pub fn on_start_turn_board(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let magnetized_gold: u32 = state.board.iter().map(|u| u.sot_gold_bonus).sum();
    state.gold += magnetized_gold;
    notify_tavern(state, |h| h.turn_start, |s, idx, f| f(s, idx, pool, rng));
    resolve_pending_effects(&mut state.board, &state.auras, rng);
}

/// Apply End-of-Turn triggers when `EndTurn` is taken, repeated per
/// [`Passive::EndOfTurnTriggers`]: Magnetized Health bonuses, then each minion's `end_of_turn`
/// hook, left to right.
pub fn on_end_turn(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let repeats = board_passive(&state.board, Passive::EndOfTurnTriggers);
    for _ in 0..repeats {
        for unit in &mut state.board {
            if unit.eot_health_bonus > 0 {
                unit.add_stats(0, unit.eot_health_bonus);
            }
        }
        notify_tavern(state, |h| h.end_of_turn, |s, idx, f| f(s, idx, pool, rng));
        resolve_pending_effects(&mut state.board, &state.auras, rng);
    }
}

/// Apply board-wide observers when a Tavern spell is cast (`Timecap'n Hooktail`, `Vicious Mindslasher`, `Charging Czarina`, `Living Azerite`, `Forsaken Weaver`, `Sha of Fear`).
pub fn on_cast_tavern_spell(state: &mut TavernState) {
    tier3::timecapn_hooktail::on_cast_tavern_spell(state);
    tier3::vicious_mindslasher::on_cast_tavern_spell(state);
    tier5::charging_czarina::on_cast_tavern_spell(state);
    tier5::living_azerite::on_cast_tavern_spell(state);
    tier6::forsaken_weaver::after_cast_tavern_spell(state);
    tier7::sha_of_fear::on_cast_tavern_spell(state);
}

/// Apply `after_targeted_spell` observers (left to right) after a spell targeted
/// `state.board[target_pos]`.
pub fn after_cast_targeted_spell(
    state: &mut TavernState,
    target_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    notify_tavern(
        state,
        |h| h.after_targeted_spell,
        |s, idx, f| {
            if target_pos < s.board.len() {
                f(s, idx, target_pos, pool, rng);
            }
        },
    );
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

/// Apply board-wide observers when Gold is spent (`Gunpowder Courier`, `Air Revenant`, `Enterprising Escapee`, `Sky Admiral Rogers`).
pub fn on_gold_spent(state: &mut TavernState, amount: u32, pool: &mut CardPool, rng: &mut Rng) {
    tier4::gunpowder_courier::on_gold_spent(state, amount);
    tier5::air_revenant::on_gold_spent(state, amount, pool, rng);
    tier5::enterprising_escapee::on_gold_spent(state, amount, rng);
    tier6::sky_admiral_rogers::on_gold_spent(state, amount, rng);
}

/// Apply `minion_bought` observers (left to right) to a minion being bought, before it reaches
/// the hand.
pub fn on_minion_bought(state: &mut TavernState, bought: &mut Unit) {
    notify_tavern(
        state,
        |h| h.minion_bought,
        |s, idx, f| f(s, idx, &mut *bought),
    );
}

/// Apply `after_buy` observers (left to right) after buying a card from the shop.
pub fn after_buy_card(state: &mut TavernState, bought: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    notify_tavern(
        state,
        |h| h.after_buy,
        |s, idx, f| f(s, idx, bought, pool, rng),
    );
}

/// Apply board-wide observers after Discovering a card (`Hooktusk, Master Marauder`).
pub fn on_card_discovered(state: &mut TavernState) {
    tier6::hooktusk_master_marauder::on_card_discovered(state);
}

/// Apply board-wide observers whenever a card is added to `hand` (`The Shadow of Doubt`).
pub fn on_card_added_to_hand(board: &[Unit], auras: &mut PlayerAuras) {
    tier6::the_shadow_of_doubt::on_card_added_to_hand(board, auras);
}

/// Return the number of extra times a Blood Gem played from hand should cast (`Hot-Air Surveyor`).
pub fn extra_hand_blood_gem_casts(board: &[Unit]) -> u32 {
    tier4::hot_air_surveyor::extra_hand_blood_gem_casts(board)
}

/// Hook called whenever `count` Blood Gems are played on `unit` (its `blood_gems_played` hook).
pub fn on_blood_gems_played_on_unit(unit: &mut Unit, count: u32) {
    if let Some(blood_gems_played) = hooks(unit.card_id).blood_gems_played {
        blood_gems_played(unit, count);
    }
}

/// Flush effects queued by minions on `board` (their `resolve_pending` hooks), left to right.
pub fn resolve_pending_effects(board: &mut [Unit], auras: &PlayerAuras, rng: &mut Rng) {
    for idx in 0..board.len() {
        if let Some(resolve_pending) = hooks(board[idx].card_id).resolve_pending {
            resolve_pending(board, idx, auras, rng);
        }
    }
}

/// Resolve discard triggers when `discarded` is discarded from hand (`Sludge Corrosion`, `Corrupted Coin`, `Energizing Chamber`, `Cutthroat K'Thir`, `Mindbender Ghur'sha`, `Harbinger Aph'lass`).
pub fn on_discard_hand_card(
    state: &mut TavernState,
    discarded: &Unit,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    state.auras.cards_discarded += 1;
    tier4::cutthroat_kthir::on_discard(state);
    tier5::mindbender_ghursha::on_discard(state);
    tier6::harbinger_aphlass::on_discard(state);
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
pub fn on_hero_damage_taken(
    board: &mut [Unit],
    shop: &mut [Unit],
    auras: &mut PlayerAuras,
) -> bool {
    let r1 = tier2::soul_rewinder::on_hero_damage_taken(board);
    let r2 = tier4::ashen_corruptor::on_hero_damage_taken(board, shop, auras);
    tier5::tichondrius::on_hero_damage_taken(board);
    r1 || r2
}

/// Returns the Gold cost of a minion's `Activate` ability, if it has one.
pub fn activate_cost(card_id: CardId) -> Option<u32> {
    hooks(card_id).activate_cost
}

/// Returns the target domain required by a minion's `Activate` ability.
pub fn activate_target_kind(card_id: CardId) -> ActivateTargetKind {
    hooks(card_id).activate_target
}

/// Execute the `Activate` ability of `state.board[source_pos]`.
pub fn on_activate(
    state: &mut TavernState,
    source_pos: usize,
    target_pos: Option<usize>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if let Some(activate) = hooks(state.board[source_pos].card_id).activate {
        activate(state, source_pos, target_pos, pool, rng);
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
        apply_combat_summon_modifiers(board, auras, combat_beast_bonus_atk, &mut beetle);
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

/// Resolve Start-of-Combat triggers for one side: hand effects (`Flighty Scout`), `Boon of
/// Beetles`, then each living minion's `start_of_combat` hook, left to right.
pub fn on_start_of_combat(ctx: &mut BoardCtx<'_>) {
    tier1::flighty_scout::on_start_of_combat(
        ctx.side,
        ctx.hand,
        ctx.board,
        ctx.auras,
        *ctx.beast_bonus_atk,
        ctx.next_id,
        ctx.events,
    );
    summon_boon_of_beetles(
        ctx.side,
        ctx.board,
        ctx.auras,
        *ctx.beast_bonus_atk,
        ctx.next_id,
        ctx.events,
    );
    let sources: Vec<(UnitId, CardId, bool)> = ctx
        .board
        .iter()
        .map(|u| (u.id, u.card_id, u.is_golden))
        .collect();
    for (id, card_id, is_golden) in sources {
        if !ctx.board.iter().any(|u| u.id == id && u.health > 0) {
            continue;
        }
        if let Some(start_of_combat) = hooks(card_id).start_of_combat {
            start_of_combat(ctx, id, is_golden);
        }
    }
}

/// Resolve a minion's On-Attack (`Rally`) hook. `def_target` is the defending board and the
/// attack target's index (`None` when the Rally is triggered in the Tavern).
/// Returns any token(s) to be summoned immediately to the attacker's right.
#[allow(clippy::too_many_arguments)]
pub fn on_rally(
    side: Side,
    board: &mut [Unit],
    attacker_pos: usize,
    def_target: Option<(&mut [Unit], usize)>,
    auras: &mut PlayerAuras,
    hand: &[Unit],
    hand_summoned: &mut [bool],
    generated_hand: &mut Vec<Unit>,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) -> Vec<Unit> {
    let attacker = &board[attacker_pos];
    let summons = match hooks(attacker.card_id).rally {
        Some(rally) => {
            let mut ctx = RallyCtx {
                side,
                in_combat: def_target.is_some(),
                attacker_id: attacker.id,
                is_golden: attacker.is_golden,
                board: &mut *board,
                attacker_pos,
                def_target,
                auras: &mut *auras,
                hand,
                hand_summoned,
                generated_hand,
                summons_attack_target: false,
                rng: &mut *rng,
                events,
            };
            rally(&mut ctx)
        }
        None => Vec::new(),
    };
    resolve_pending_effects(board, auras, rng);
    summons
}

/// Trigger a friendly minion's `Rally` effect on `state.board[target_pos]` during the Tavern Phase (`Sky-hatch Runaway`).
pub fn trigger_tavern_rally(state: &mut TavernState, target_pos: usize, rng: &mut Rng) {
    if target_pos >= state.board.len() {
        return;
    }
    let is_rally = is_rally_minion(state.board[target_pos].card_id);
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
            on_tavern_summon(&mut state.board, insert_pos);
            insert_pos += 1;
            state.check_and_resolve_triple(cid);
        }
    }
    if is_rally && target_pos < state.board.len() {
        let old_tavern_all = (state.auras.tavern_all_atk, state.auras.tavern_all_hp);
        let mut deathstrider_hand = state.hand.clone();
        let prev_hand_len = deathstrider_hand.len();
        let mut hand_summoned2 = vec![false; prev_hand_len];
        let mut beast_bonus_atk = 0;
        let mut pending_attacks = Vec::new();
        let mut next_id = 10_000;
        let mut ctx = BoardCtx {
            side: Side::A,
            in_combat: false,
            board: &mut state.board,
            cursor: 0,
            auras: &mut state.auras,
            hand: &mut deathstrider_hand,
            hand_summoned: &mut hand_summoned2,
            dead_aberrations: &[],
            beast_bonus_atk: &mut beast_bonus_atk,
            hero_tier: state.tavern_tier,
            pending_attacks: &mut pending_attacks,
            enemy_destroys: Vec::new(),
            next_id: &mut next_id,
            rng,
            events: &mut events,
        };
        tier6::deathstrider::after_rally_minion_attacks(&mut ctx);
        for card in deathstrider_hand.into_iter().skip(prev_hand_len) {
            state.add_to_hand(card);
        }
        let d_atk = state.auras.tavern_all_atk - old_tavern_all.0;
        let d_hp = state.auras.tavern_all_hp - old_tavern_all.1;
        if d_atk != 0 || d_hp != 0 {
            for shop_unit in state.shop.iter_mut() {
                if !shop_unit.is_spell {
                    shop_unit.add_stats(d_atk, d_hp);
                }
            }
        }
    }
    state.sync_all_auras();
}

/// Resolve board-wide observers when a friendly minion attacks (`Prodigious Tusker`, `Roaring Recruiter`, `Cage Gnawer`, `Ravaging Scorpid`).
pub fn on_friendly_attack(
    side: Side,
    board: &mut [Unit],
    attacker_id: UnitId,
    auras: &mut PlayerAuras,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    tier2::prodigious_tusker::on_friendly_attack(side, board, attacker_id, auras, events);
    tier3::roaring_recruiter::on_friendly_attack(side, board, attacker_id, events);
    tier4::cage_gnawer::on_friendly_attack(side, board, attacker_id, events);
    tier6::ravaging_scorpid::on_friendly_attack(board, auras);
    resolve_pending_effects(board, auras, rng);
}

/// Resolve on-damage-dealt triggers (`Treasure Parrot`, `Devout Hellcaller`).
#[allow(clippy::too_many_arguments)]
pub fn on_damage_dealt(
    side: Side,
    board: &mut [Unit],
    source_id: UnitId,
    amount: i32,
    auras: &mut PlayerAuras,
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
    let prev_hand_len = hand.len();
    tier3::treasure_parrot::on_dealt_damage(
        &mut board[src_idx],
        amount,
        hand,
        hand_summoned,
    );
    for _ in prev_hand_len..hand.len() {
        on_card_added_to_hand(board, auras);
    }
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

/// Resolve all friendly-death observers and `Avenge` triggers during combat (`Eternal Knight`, `Relentless Deflector`, `Drustfallen Butcher`, `Lichling Hoarder`, `Eternal Tycoon`, `Deathly Striker`).
#[allow(clippy::too_many_arguments)]
pub fn on_combat_friendly_death(
    side: Side,
    dying: &Unit,
    surviving_board: &mut Vec<Unit>,
    auras: &mut PlayerAuras,
    hero_tier: u32,
    hand: &mut Vec<Unit>,
    hand_summoned: &mut Vec<bool>,
    combat_beast_bonus_atk: i32,
    next_id: &mut UnitId,
    pending_immediate_attacks: &mut Vec<UnitId>,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    on_unit_died(dying, surviving_board, auras);
    let prev_hand_len = hand.len();
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
    for _ in prev_hand_len..hand.len() {
        on_card_added_to_hand(surviving_board, auras);
    }
    let striker_indices: Vec<usize> = surviving_board
        .iter()
        .enumerate()
        .filter(|(_, u)| u.card_id == tier6::deathly_striker::ID)
        .map(|(i, _)| i)
        .collect();
    for idx in striker_indices {
        surviving_board[idx].avenge_counter += 1;
        while surviving_board[idx].avenge_counter >= 4 {
            surviving_board[idx].avenge_counter -= 4;
            let is_golden = surviving_board[idx].is_golden;
            tier6::deathly_striker::on_avenge(
                is_golden,
                hero_tier,
                surviving_board,
                auras,
                hand,
                hand_summoned,
                rng,
            );
        }
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

/// Resolve a dying minion's `Deathrattle` hook (once; callers apply repeat multipliers).
pub fn on_deathrattle(dying: &Unit, ctx: &mut BoardCtx<'_>) {
    if let Some(deathrattle) = hooks(dying.card_id).deathrattle {
        ctx.auras.deathrattles_triggered += 1;
        deathrattle(dying, ctx);
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
