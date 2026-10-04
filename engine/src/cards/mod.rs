//! Card templates, catalogs, and generic per-card hook dispatch.
//!
//! Card definitions are organized by folder (`src/cards/tier1/` .. `src/cards/tier7/`,
//! `src/cards/deities.rs`, `src/cards/spells.rs`, `src/cards/tokens.rs`). Each card declares
//! its behaviour as a [`CardHooks`] table (see [`hooks`](mod@hooks)); the dispatch functions
//! in this module look hooks up by `CardId` through the [registry](fn@hooks) and never name a
//! specific card.

pub mod deities;
mod effects;
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

pub use deities::{instantiate_deity, DEITY_SACRIFICE_REQUIREMENT};
pub use hooks::{CardFlags, CardHooks, Passive, Played, SocAction};
pub use registry::{all_templates, hooks, template};

use registry::plain_instance;

use crate::combat::MAX_BOARD_SIZE;
use crate::events::Event;
use crate::model::{CardId, Keyword, PlayerAuras, Side, Tribe, Unit, UnitId, BONUS_KEYWORDS};
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

/// Board index of the first (left-most) minion whose card has `flag` and that has `charges`
/// left.
fn charged_unit(board: &[Unit], flag: CardFlags) -> Option<usize> {
    board
        .iter()
        .position(|u| u.charges > 0 && hooks(u.card_id).has(flag))
}

/// `true` if a minion on `board` whose card has `flag` has `charges` left.
pub fn has_charge(board: &[Unit], flag: CardFlags) -> bool {
    charged_unit(board, flag).is_some()
}

/// Use a charge of the first (left-most) minion on `board` whose card has `flag` and that has
/// `charges` left (`false` if there is none).
pub fn use_charge(board: &mut [Unit], flag: CardFlags) -> bool {
    let Some(i) = charged_unit(board, flag) else {
        return false;
    };
    board[i].charges -= 1;
    true
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

/// The spell aura `unit` has on top of its card's own `spell_aura` (e.g. from Magnetized
/// `Enchanted Sentinel`s).
pub fn gained_spell_aura(unit: &Unit) -> (i32, i32) {
    let (atk, hp) = hooks(unit.card_id).spell_aura;
    let m = unit.golden_mult();
    (
        unit.spell_atk_aura - unit.spell_atk_aura.min(atk * m),
        unit.spell_hp_aura - unit.spell_hp_aura.min(hp * m),
    )
}

/// A Golden was merged from the triple `copies`: let its card adjust card-specific state (its
/// `merge_golden` hook).
pub fn on_merge_golden(copies: &[Unit], golden: &mut Unit) {
    if let Some(merge_golden) = hooks(golden.card_id).merge_golden {
        merge_golden(copies, golden);
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

/// Run the unit's stat-threshold checks (its `stat_threshold` hook) whenever its stats change
/// (in Tavern or Combat).
pub fn check_stat_thresholds(unit: &mut Unit) {
    if let Some(stat_threshold) = hooks(unit.card_id).stat_threshold {
        stat_threshold(unit);
    }
}

/// Synchronize a unit's persistent "wherever this is" auras: its aura stat bonus (player-wide
/// effects plus its card's `aura_bonus`, tracked in `aura_applied`), its board-aura spell bonus,
/// and its card's own `sync_aura` hook.
pub fn sync_unit_auras(unit: &mut Unit, auras: &PlayerAuras) {
    if unit.is_spell {
        return;
    }
    let card = hooks(unit.card_id);
    let (mut atk, mut hp) = effects::aura_bonus(unit, auras);
    if let Some(aura_bonus) = card.aura_bonus {
        let (card_atk, card_hp) = aura_bonus(unit, auras);
        atk += card_atk;
        hp += card_hp;
    }
    let (d_atk, d_hp) = (atk - unit.aura_applied.0, hp - unit.aura_applied.1);
    if d_atk != 0 || d_hp != 0 {
        unit.aura_applied = (atk, hp);
        unit.add_stats(d_atk, d_hp);
    }
    if unit.spell_atk_aura == 0 && unit.spell_hp_aura == 0 {
        init_spell_aura(unit);
    }
    if let Some(sync_aura) = card.sync_aura {
        sync_aura(unit, auras);
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
    let mut copy = if let Some(mut u) = plain_instance(dying) {
        if dying.is_golden {
            u.make_golden();
            u.intrinsic_golden = dying.intrinsic_golden;
        }
        u
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

/// Extra times the next Magnetization onto `unit` happens, consuming what its card queued (its
/// `extra_magnetizations` hook; 0 if none).
pub fn extra_magnetizations(unit: &mut Unit) -> u32 {
    hooks(unit.card_id)
        .extra_magnetizations
        .map_or(0, |f| f(unit))
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
    let repeats = 1 + extra_magnetizations(&mut state.board[target_pos]);
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

/// Apply On-Sell triggers when `sold` is sold from the board: `after_friendly_sell` observers
/// (left to right), then the sold card's own `sell` hook.
pub fn on_sell(state: &mut TavernState, sold: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    sync_board_spell_auras(&state.board, &mut state.auras);
    notify_tavern(
        state,
        |h| h.after_friendly_sell,
        |s, idx, f| f(s, idx, pool, rng),
    );
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

/// Apply `spell_cast` observers (left to right) when a Tavern spell is cast.
pub fn on_cast_tavern_spell(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    notify_tavern(state, |h| h.spell_cast, |s, idx, f| f(s, idx, pool, rng));
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

/// Apply `after_spell_cast` observers (left to right) after any spell is cast.
pub fn after_cast_any_spell(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    notify_tavern(
        state,
        |h| h.after_spell_cast,
        |s, idx, f| f(s, idx, pool, rng),
    );
}

/// Apply `gold_spent` observers (left to right) after `amount` Gold is spent.
pub fn on_gold_spent(state: &mut TavernState, amount: u32, pool: &mut CardPool, rng: &mut Rng) {
    notify_tavern(
        state,
        |h| h.gold_spent,
        |s, idx, f| f(s, idx, amount, pool, rng),
    );
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

/// Apply `discover` observers (left to right) after a card is Discovered.
pub fn on_card_discovered(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    notify_tavern(state, |h| h.discover, |s, idx, f| f(s, idx, pool, rng));
}

/// After a shop `Refresh` (including the start-of-turn one): resolve the player's Refresh
/// effects on the new shop.
pub fn after_shop_refresh(state: &mut TavernState, rng: &mut Rng) {
    effects::after_shop_refresh(state, rng);
}

/// Start-of-turn upkeep of the hand card at `state.hand[hand_idx]` (its `turn_start_in_hand`
/// hook).
pub fn on_turn_start_in_hand(state: &mut TavernState, hand_idx: usize, rng: &mut Rng) {
    if let Some(turn_start_in_hand) = hooks(state.hand[hand_idx].card_id).turn_start_in_hand {
        turn_start_in_hand(state, hand_idx, rng);
    }
}

/// Let hand cards that wait for a condition resolve once it holds (`ready_in_hand` hooks, left
/// to right; e.g. a `Lockbox` whose countdown ended).
pub fn resolve_ready_hand_cards(state: &mut TavernState, rng: &mut Rng) {
    let mut idx = 0;
    while idx < state.hand.len() {
        if let Some(ready_in_hand) = hooks(state.hand[idx].card_id).ready_in_hand {
            ready_in_hand(state, idx, rng);
        }
        idx += 1;
    }
}

/// Returns `true` if `card_id` is an option card of a Choose-One / Discover-style prompt that
/// takes effect when chosen (it has a `choose` hook) instead of going to the hand.
pub fn is_option_card(card_id: CardId) -> bool {
    hooks(card_id).choose.is_some()
}

/// Apply the option card `chosen`, picked from a Choose-One / Discover-style prompt (its
/// `choose` hook).
pub fn apply_chosen_option(
    state: &mut TavernState,
    chosen: &Unit,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if let Some(choose) = hooks(chosen.card_id).choose {
        choose(state, chosen, pool, rng);
    }
}

/// Apply `after_friendly_choose_one` observers (left to right) after a Choose One resolved with
/// `option` (Discover-style options without [`CardFlags::CHOOSE_ONE_OPTION`] don't count).
pub fn after_choose_one(
    state: &mut TavernState,
    option: &Unit,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if hooks(option.card_id).has(CardFlags::CHOOSE_ONE_OPTION) {
        notify_tavern(
            state,
            |h| h.after_friendly_choose_one,
            |s, idx, f| f(s, idx, pool, rng),
        );
    }
}

/// Apply `card_added_to_hand` observers (living minions on `board`, left to right) after a card
/// is added to the hand.
pub fn on_card_added_to_hand(board: &[Unit], auras: &mut PlayerAuras) {
    for unit in board.iter().filter(|u| u.health > 0) {
        if let Some(card_added_to_hand) = hooks(unit.card_id).card_added_to_hand {
            card_added_to_hand(unit, auras);
        }
    }
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

/// Resolve a discard from hand: `after_friendly_discard` observers (left to right), then the
/// discarded card's own `discarded` hook.
pub fn on_discard_hand_card(
    state: &mut TavernState,
    discarded: &Unit,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    state.auras.cards_discarded += 1;
    notify_tavern(
        state,
        |h| h.after_friendly_discard,
        |s, idx, f| f(s, idx, pool, rng),
    );
    if let Some(on_discarded) = hooks(discarded.card_id).discarded {
        on_discarded(state, discarded, pool, rng);
    }
}

/// Returns `true` if a minion on `board` rewinds hero damage taken in the Tavern
/// ([`CardFlags::REWINDS_HERO_DAMAGE`]).
pub fn board_prevents_hero_damage(board: &[Unit]) -> bool {
    board
        .iter()
        .any(|u| hooks(u.card_id).has(CardFlags::REWINDS_HERO_DAMAGE))
}

/// Apply `hero_damage` observers (left to right) before the hero takes `amount` Tavern damage.
/// Returns `true` if the damage is rewound.
pub fn on_hero_damage(state: &mut TavernState, amount: i32) -> bool {
    let rewound = board_prevents_hero_damage(&state.board);
    notify_tavern(state, |h| h.hero_damage, |s, idx, f| f(s, idx, amount));
    rewound
}

/// Apply `after_hero_damage` observers (left to right) after the hero took `amount` damage.
pub fn after_hero_damage(state: &mut TavernState, amount: i32) {
    notify_tavern(
        state,
        |h| h.after_hero_damage,
        |s, idx, f| f(s, idx, amount),
    );
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

/// Add the copies that hand cards summon at Start of Combat (`combat_copies_from_hand`) to
/// `board` (a preview of the combat board).
pub fn on_start_of_combat_hand(hand: &[Unit], board: &mut Vec<Unit>) {
    for card in hand {
        for _ in 0..combat_copies_from_hand(card) {
            if board.len() < MAX_BOARD_SIZE {
                board.push(card.clone());
            }
        }
    }
}

/// Number of copies of the hand card `card` it summons at Start of Combat.
fn combat_copies_from_hand(card: &Unit) -> u32 {
    hooks(card.card_id)
        .combat_copies_from_hand
        .map_or(0, |f| f(card))
}

/// Summon the copies that hand cards summon at Start of Combat (`combat_copies_from_hand`) at
/// the end of the board.
fn summon_copies_from_hand(ctx: &mut BoardCtx<'_>) {
    for hand_idx in 0..ctx.hand.len() {
        for _ in 0..combat_copies_from_hand(&ctx.hand[hand_idx]) {
            if ctx.board.len() >= MAX_BOARD_SIZE {
                break;
            }
            let mut copy = ctx.hand[hand_idx].clone();
            copy.id = *ctx.next_id;
            *ctx.next_id += 1;
            sync_unit_auras(&mut copy, ctx.auras);
            if copy.tribe.matches(Tribe::Beast) && *ctx.beast_bonus_atk != 0 {
                copy.add_stats(*ctx.beast_bonus_atk, 0);
            }
            copy.sync_max_stats();
            check_stat_thresholds(&mut copy);
            let reason = template(copy.card_id).map_or("Start of Combat", |t| t.name.as_str());
            ctx.events.push(Event::UnitSummoned {
                side: ctx.side,
                source: copy.id,
                unit: copy.id,
                name: copy.name.clone(),
                attack: copy.attack,
                health: copy.health,
                reason,
            });
            ctx.board.push(copy);
        }
    }
}

/// Resolve Start-of-Combat triggers for one side: copies summoned from hand
/// (`combat_copies_from_hand`), player-level effects (e.g. `Boon of Beetles`), then each living
/// minion's `start_of_combat` hook, left to right.
pub fn on_start_of_combat(ctx: &mut BoardCtx<'_>) {
    summon_copies_from_hand(ctx);
    effects::start_of_combat(ctx);
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

/// The engine-resolved Start-of-Combat action of `unit` (its `start_of_combat_action` hook).
pub fn start_of_combat_action(unit: &Unit) -> Option<SocAction> {
    hooks(unit.card_id).start_of_combat_action.map(|f| f(unit))
}

/// Resolve the `awaken` hook of the Deity that just awakened at `ctx.board[idx]`.
pub fn on_awaken(ctx: &mut BoardCtx<'_>, idx: usize) {
    if let Some(awaken) = hooks(ctx.board[idx].card_id).awaken {
        awaken(ctx, idx);
    }
}

/// Resolve a minion's On-Attack (`Rally`) hook. `def_target` is the defending board and the
/// attack target's index (`None` when the Rally is triggered in the Tavern).
/// Returns the token(s) to summon immediately to the attacker's right, and whether they attack
/// the target first ([`RallyCtx::summons_attack_target`]).
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
) -> (Vec<Unit>, bool) {
    let attacker = &board[attacker_pos];
    let rallied = match hooks(attacker.card_id).rally {
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
            let summons = rally(&mut ctx);
            (summons, ctx.summons_attack_target)
        }
        None => (Vec::new(), false),
    };
    resolve_pending_effects(board, auras, rng);
    rallied
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
    let (summons, _) = on_rally(
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
        after_friendly_rally(&mut ctx);
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

/// Notify friendly units after a friendly Rally minion attacked (`after_friendly_rally` hooks,
/// left to right).
pub fn after_friendly_rally(ctx: &mut BoardCtx<'_>) {
    notify_board(ctx, |h| h.after_friendly_rally, |c, idx, f| f(c, idx));
}

/// Notify friendly units that the friendly minion `attacker_id` attacks (`friendly_attack`
/// hooks, left to right), then flush queued effects.
pub fn on_friendly_attack(ctx: &mut BoardCtx<'_>, attacker_id: UnitId) {
    notify_board(
        ctx,
        |h| h.friendly_attack,
        |c, idx, f| f(c, idx, attacker_id),
    );
    resolve_pending_effects(ctx.board, ctx.auras, ctx.rng);
}

/// Resolve damage-dealt triggers after the friendly unit `source_id` dealt `amount` damage: its
/// own `damage_dealt` hook, then the other friendly units' `after_friendly_damage_dealt` hooks.
pub fn on_damage_dealt(ctx: &mut BoardCtx<'_>, source_id: UnitId, amount: i32) {
    if amount <= 0 {
        return;
    }
    let Some(src_idx) = ctx.board.iter().position(|u| u.id == source_id) else {
        return;
    };
    let (src_card, src_tribe) = (ctx.board[src_idx].card_id, ctx.board[src_idx].tribe);
    if let Some(damage_dealt) = hooks(src_card).damage_dealt {
        damage_dealt(ctx, src_idx, amount);
    }
    notify_board(
        ctx,
        |h| h.after_friendly_damage_dealt,
        |c, idx, f| {
            if c.board[idx].id != source_id {
                f(c, idx, source_id, src_tribe);
            }
        },
    );
}

/// Resolve `unit`'s own `damage_taken` hook after it took damage (`hand` is its owner's hand).
pub fn on_damage_taken(unit: &Unit, hand: &mut [Unit], rng: &mut Rng) {
    if let Some(damage_taken) = hooks(unit.card_id).damage_taken {
        damage_taken(unit, hand, rng);
    }
}

/// Returns `true` if `card_id`'s attacks also damage the enemies adjacent to the target
/// ([`CardFlags::CLEAVE`]).
pub fn cleaves_adjacent_enemies(card_id: CardId) -> bool {
    hooks(card_id).has(CardFlags::CLEAVE)
}

/// Returns `true` if `card_id` deals excess attack damage to the target's neighbour(s)
/// ([`CardFlags::EXCESS_DAMAGE_TO_NEIGHBORS`]).
pub fn deals_excess_damage_to_neighbors(card_id: CardId) -> bool {
    hooks(card_id).has(CardFlags::EXCESS_DAMAGE_TO_NEIGHBORS)
}

/// Bonus damage `attacker` deals to the highest-Health enemy after it attacks
/// (`after_attack_damage` hook; 0 = none).
pub fn after_attack_damage(attacker: &Unit) -> i32 {
    hooks(attacker.card_id)
        .after_attack_damage
        .map_or(0, |f| f(attacker))
}

/// Pop `board[idx]`'s Divine Shield, then notify the living units on `board` that carry a
/// `friendly_divine_shield_lost` hook.
pub fn pop_divine_shield(board: &mut [Unit], idx: usize, events: &mut Vec<Event>) {
    board[idx].divine_shield = false;
    events.push(Event::DivineShieldPopped {
        unit: board[idx].id,
    });
    for unit in board.iter_mut().filter(|u| u.health > 0) {
        if let Some(divine_shield_lost) = hooks(unit.card_id).friendly_divine_shield_lost {
            divine_shield_lost(unit);
        }
    }
}

/// Resolve a friendly minion's death for its side (Tavern or Combat): the dying card's own
/// `died` hook, then the surviving units' `friendly_death` hooks (e.g. Avenge), left to right.
pub fn on_friendly_death(ctx: &mut BoardCtx<'_>, dying: &Unit) {
    if let Some(died) = hooks(dying.card_id).died {
        died(dying, ctx.auras);
    }
    notify_board(ctx, |h| h.friendly_death, |c, idx, f| f(c, idx, dying));
}

/// Resolve a dying minion's `Deathrattle` hook (once; callers apply repeat multipliers).
pub fn on_deathrattle(dying: &Unit, ctx: &mut BoardCtx<'_>) {
    if let Some(deathrattle) = hooks(dying.card_id).deathrattle {
        ctx.auras.deathrattles_triggered += 1;
        deathrattle(dying, ctx);
    }
}

/// Resolve effects that trigger after a round of combat deaths resolved for one side (player-level
/// effects that fill open board slots, e.g. `Boon of Beetles`).
pub fn after_combat_deaths(ctx: &mut BoardCtx<'_>) {
    effects::after_combat_deaths(ctx);
}

/// Synchronize dynamic combat/player auras across `board` and `hand` after deaths resolve.
pub fn sync_combat_auras(board: &mut [Unit], hand: &mut [Unit], auras: &PlayerAuras) {
    for u in board.iter_mut() {
        sync_unit_auras(u, auras);
    }
    for h in hand.iter_mut() {
        sync_unit_auras(h, auras);
    }
}

/// `(id, attack, health)` of every unit on `board`, for [`push_stat_changes`].
pub fn stat_snapshot(board: &[Unit]) -> Vec<(UnitId, i32, i32)> {
    board.iter().map(|u| (u.id, u.attack, u.health)).collect()
}

/// Emit a `StatBuff` event (logged with `reason`) for every unit on `board` whose stats differ
/// from `before`, a [`stat_snapshot`] of the same board (compared slot by slot).
pub fn push_stat_changes(
    side: Side,
    board: &[Unit],
    before: &[(UnitId, i32, i32)],
    reason: &'static str,
    events: &mut Vec<Event>,
) {
    for (u, &(id, old_atk, old_hp)) in board.iter().zip(before) {
        if u.attack != old_atk || u.health != old_hp {
            events.push(Event::StatBuff {
                side,
                unit: id,
                atk_delta: u.attack - old_atk,
                hp_delta: u.health - old_hp,
                attack: u.attack,
                health: u.health,
                reason,
            });
        }
    }
}

/// Permanently give `tavern_unit` the stats (times `mult`) and Bonus Keywords its combat copy
/// gained in combat (`pre` / `post`: the combat copy before and after combat).
pub fn keep_combat_gains(pre: &Unit, post: &Unit, tavern_unit: &mut Unit, mult: i32) {
    let atk_gain = (post.max_attack - pre.attack).max(0) * mult;
    let hp_gain = (post.max_health - pre.health).max(0) * mult;
    if atk_gain > 0 || hp_gain > 0 {
        tavern_unit.add_stats(atk_gain, hp_gain);
    }
    for kw in BONUS_KEYWORDS {
        if post.has_keyword(kw) || (kw == Keyword::DivineShield && post.inherent_divine_shield) {
            tavern_unit.apply_keyword(kw, kw == Keyword::DivineShield);
        }
    }
    check_stat_thresholds(tavern_unit);
}

/// Apply post-combat persistence from a combat unit back to its Tavern counterpart: the card's
/// own `post_combat` hook, combat gains kept through its neighbours'
/// `post_combat_neighbor_mult` hooks, then permanent gains and improvements (`stacks`).
pub fn on_post_combat_unit(
    pre_board: &[Unit],
    idx: usize,
    post_combat_board: &[Unit],
    tavern_unit: &mut Unit,
) {
    let Some(pre_combat) = pre_board.get(idx) else {
        return;
    };
    if let Some(post_combat) = hooks(tavern_unit.card_id).post_combat {
        post_combat(pre_combat, post_combat_board, tavern_unit);
    }
    let Some(post) = post_combat_board.iter().find(|u| u.id == pre_combat.id) else {
        return;
    };
    let keep_mult = [idx.checked_sub(1), Some(idx + 1)]
        .into_iter()
        .flatten()
        .filter_map(|n| {
            let neighbor_mult = hooks(pre_board.get(n)?.card_id).post_combat_neighbor_mult?;
            Some(neighbor_mult(pre_board, n, idx))
        })
        .max()
        .unwrap_or(0);
    if keep_mult > 0 {
        keep_combat_gains(pre_combat, post, tavern_unit, keep_mult);
    }
    if post.perm_atk_gained != 0 || post.perm_hp_gained != 0 {
        tavern_unit.add_stats(post.perm_atk_gained, post.perm_hp_gained);
    }
    if post.perm_blood_gems_gained > 0 {
        tavern_unit.blood_gems_played += post.perm_blood_gems_gained;
        tavern_unit.blood_gem_stats_applied.0 += post.perm_atk_gained;
        tavern_unit.blood_gem_stats_applied.1 += post.perm_hp_gained;
    }
    tavern_unit.stacks = post.stacks;
}
