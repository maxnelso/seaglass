//! Core data types: units, tribes, keywords, deities, player auras, and game state.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Stable identity for a unit within a single battle (`docs/combat.md` §2).
pub type UnitId = u32;

/// Unique catalog identifier for a card template.
pub type CardId = u32;

/// Supported minion keywords (`docs/combat.md` §1).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Keyword {
    Taunt,
    DivineShield,
    Windfury,
    Reborn,
    Venomous,
    Stealth,
    Magnetic,
}

/// Bonus keywords eligible to be rolled by effects like `Bubble Gunner`.
pub const BONUS_KEYWORDS: [Keyword; 6] = [
    Keyword::Taunt,
    Keyword::DivineShield,
    Keyword::Windfury,
    Keyword::Reborn,
    Keyword::Venomous,
    Keyword::Stealth,
];

/// Minion tribe classification (Patch 36.6.3), including dual-tribe combinations.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Tribe {
    #[default]
    None,
    Aberration,
    Beast,
    Demon,
    Dragon,
    Elemental,
    Mech,
    Murloc,
    Pirate,
    Quilboar,
    Undead,
    UndeadMech,
    DragonPirate,
    BeastPirate,
    ElementalDemon,
    MechMurloc,
    DemonDragon,
    DemonQuilboar,
    All,
}

/// All 10 base single minion tribes in Patch 36.6.3.
pub const SINGLE_TRIBES: [Tribe; 10] = [
    Tribe::Aberration,
    Tribe::Beast,
    Tribe::Demon,
    Tribe::Dragon,
    Tribe::Elemental,
    Tribe::Mech,
    Tribe::Murloc,
    Tribe::Pirate,
    Tribe::Quilboar,
    Tribe::Undead,
];

impl Tribe {
    /// Returns `true` if this tribe classification includes the given base `single` tribe.
    #[inline]
    pub fn has_single(self, single: Tribe) -> bool {
        match self {
            Tribe::None => false,
            Tribe::All => matches!(
                single,
                Tribe::Aberration
                    | Tribe::Beast
                    | Tribe::Demon
                    | Tribe::Dragon
                    | Tribe::Elemental
                    | Tribe::Mech
                    | Tribe::Murloc
                    | Tribe::Pirate
                    | Tribe::Quilboar
                    | Tribe::Undead
            ),
            Tribe::UndeadMech => matches!(single, Tribe::Undead | Tribe::Mech),
            Tribe::DragonPirate => matches!(single, Tribe::Dragon | Tribe::Pirate),
            Tribe::BeastPirate => matches!(single, Tribe::Beast | Tribe::Pirate),
            Tribe::ElementalDemon => matches!(single, Tribe::Elemental | Tribe::Demon),
            Tribe::MechMurloc => matches!(single, Tribe::Mech | Tribe::Murloc),
            Tribe::DemonDragon => matches!(single, Tribe::Demon | Tribe::Dragon),
            Tribe::DemonQuilboar => matches!(single, Tribe::Demon | Tribe::Quilboar),
            other => other == single,
        }
    }

    /// Returns `true` if `self` and `target` share at least one minion tribe.
    /// `Tribe::None` matches nothing (not even `Tribe::All`).
    pub fn matches(self, target: Tribe) -> bool {
        if self == Tribe::None || target == Tribe::None {
            return false;
        }
        SINGLE_TRIBES
            .into_iter()
            .any(|single| self.has_single(single) && target.has_single(single))
    }

    /// Canonical lowercase string name for this tribe.
    pub fn as_str(self) -> &'static str {
        match self {
            Tribe::None => "none",
            Tribe::Aberration => "aberration",
            Tribe::Beast => "beast",
            Tribe::Demon => "demon",
            Tribe::Dragon => "dragon",
            Tribe::Elemental => "elemental",
            Tribe::Mech => "mech",
            Tribe::Murloc => "murloc",
            Tribe::Pirate => "pirate",
            Tribe::Quilboar => "quilboar",
            Tribe::Undead => "undead",
            Tribe::UndeadMech => "undead_mech",
            Tribe::DragonPirate => "dragon_pirate",
            Tribe::BeastPirate => "beast_pirate",
            Tribe::ElementalDemon => "elemental_demon",
            Tribe::MechMurloc => "mech_murloc",
            Tribe::DemonDragon => "demon_dragon",
            Tribe::DemonQuilboar => "demon_quilboar",
            Tribe::All => "all",
        }
    }
}

/// Which Old God Deity answers the player's call in an Aberration game (Patch 36.6.1 / 36.6.3).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DeityKind {
    None,
    #[default]
    #[serde(rename = "cthun", alias = "c_thun")]
    CThun,
    #[serde(rename = "yshaarj", alias = "y_shaarj")]
    YShaarj,
}

/// Persistent state of a player's Deity across the game.
///
/// Awakens in combat after 4 friendly Aberration deaths (`docs/combat.md`, Patch 36.6.3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeityState {
    pub kind: DeityKind,
    pub attack: i32,
    pub health: i32,
    pub is_golden: bool,
}

impl Default for DeityState {
    fn default() -> Self {
        Self {
            kind: DeityKind::CThun,
            attack: 1,
            health: 1,
            is_golden: false,
        }
    }
}

impl DeityState {
    pub fn new(kind: DeityKind) -> Self {
        Self {
            kind,
            attack: 1,
            health: 1,
            is_golden: false,
        }
    }
}

/// How long a [`PlayerEffect`] lasts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectDuration {
    /// Until its card's hooks use up its stacks.
    Game,
    /// Until the start of the next turn (or until its card's hooks use up its stacks).
    Turn,
    /// For the next `n` shop Refreshes.
    Refreshes(u32),
}

/// A player-level effect recorded by card text (e.g. a Tavern spell's "next combat" effect)
/// that acts through its card's `player_*` hooks. `stacks` is how many times it was recorded
/// (or how much of it is left); an effect ends when it has no stacks left.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerEffect {
    /// The card whose hooks implement the effect.
    pub card_id: CardId,
    pub stacks: u32,
    pub duration: EffectDuration,
}

/// Outcome of a combat for one player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatResult {
    Won,
    Lost,
    Tied,
}

/// Persistent game-long scaling counters and auras (`docs/tavern.md` §7.6).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerAuras {
    /// Bonus Attack/Health applied to Elementals in the Tavern (`Dune Dweller`).
    pub tavern_elemental_atk: i32,
    pub tavern_elemental_hp: i32,
    /// Bonus Attack/Health applied to all minions in the Tavern (`Staff of Enrichment`).
    pub tavern_all_atk: i32,
    pub tavern_all_hp: i32,
    /// Bonus Attack/Health applied to all minions in the Tavern this turn (`Ashen Corruptor`).
    pub tavern_turn_atk: i32,
    pub tavern_turn_hp: i32,
    /// Bonus Attack applied to friendly Undead (`Nerubian Deathswarmer`).
    pub undead_bonus_attack: i32,
    /// Bonus Attack/Health added to Blood Gems (`Gem Day` / `Crater Miner` / `Fearless Foodie`).
    pub blood_gem_bonus_atk: i32,
    pub blood_gem_bonus_hp: i32,
    /// Extra Attack/Health granted by Tavern spells that give stats (`Intrepid Botanist`, `Azsharan Cutlassier`, `Blue Whelp`, `Shoalfin Mystic`).
    pub spell_bonus_atk: i32,
    pub spell_bonus_hp: i32,
    /// Free shop Refreshes remaining (`Leaf Through the Pages` / `Sly Infiltrator`).
    pub free_refreshes: u32,
    /// Permanent bonus to maximum Gold (`Strike Oil`).
    pub base_max_gold_bonus: u32,
    /// Total spells cast this game.
    pub spells_played: u32,
    /// Cost reduction on the next Tavern spell bought (`Ominous Seer`).
    pub next_spell_discount: i32,
    /// Per-tribe Tavern shop buffs (`Eonar's Favor`).
    pub tavern_tribe_buffs: Vec<(Tribe, i32, i32)>,
    /// Buffs given to a random minion in the Tavern after each `Refresh`, in the order they were
    /// gained (`Easterly Winds`, `En-Djinn Blazer`, `Waveling`).
    pub refresh_random_buffs: Vec<(i32, i32)>,
    /// Blood Gems played on every minion in the Tavern after each `Refresh` (`Blood Gem Barrage`).
    pub refresh_blood_gems: u32,
    /// Total cards discarded from hand this game (`Parasitic Fleshling`).
    pub cards_discarded: u32,
    /// Total Golden minions played this game (`Maritime Extortionist`).
    pub golden_minions_played: u32,
    /// The hero's current Health, kept in sync by the Tavern (`None` outside of a game, e.g. in
    /// standalone combats).
    pub hero_health: Option<i32>,
    /// Currently applied board-aura contribution to `(spell_bonus_atk, spell_bonus_hp)` (`Enchanted Sentinel`, `Humon'gozz`).
    pub board_spell_bonus_applied: (i32, i32),
    /// Card ID of the last Tavern spell cast (`Cataclysmic Harbinger`).
    pub last_tavern_spell_cast: Option<CardId>,
    /// Discovered Hero Power ID (`Unmasked Identity`).
    pub hero_power_id: u32,
    /// Total Deathrattles triggered this game (`Falling Sky Golem`).
    pub deathrattles_triggered: u32,
    /// Player's Old God Deity state (awakens after 4 friendly Aberration deaths in combat).
    pub deity: DeityState,
    /// Per-card "this game" counters that card text records and reads, keyed by the card that
    /// owns them (`Eternal Knight` deaths, `Baller` improvements, the `Beetle` stat bonus, ...).
    /// The engine never interprets them; see [`Self::counter`] and [`Self::counter_pair`].
    pub card_counters: BTreeMap<CardId, (i32, i32)>,
    /// Player-level effects recorded by card text, in the order they were recorded (see
    /// [`PlayerEffect`] and [`Self::add_effect`]).
    pub effects: Vec<PlayerEffect>,
}

impl PlayerAuras {
    /// Current stats granted by a single `Blood Gem` (`+1/+1` plus `Gem Day` bonuses).
    #[inline]
    pub fn blood_gem_stats(&self) -> (i32, i32) {
        (1 + self.blood_gem_bonus_atk, 1 + self.blood_gem_bonus_hp)
    }

    /// Current stats granted by a Tavern spell with base stat buff `(base_atk, base_hp)`.
    #[inline]
    pub fn spell_stat_buff(&self, base_atk: i32, base_hp: i32) -> (i32, i32) {
        (base_atk + self.spell_bonus_atk, base_hp + self.spell_bonus_hp)
    }

    /// Value of the card counter `key` (0 if unset).
    pub fn counter(&self, key: CardId) -> i32 {
        self.counter_pair(key).0
    }

    /// Add `n` to the card counter `key`.
    pub fn add_counter(&mut self, key: CardId, n: i32) {
        self.add_counter_pair(key, (n, 0));
    }

    /// Value of the two-part (e.g. Attack/Health) card counter `key` (`(0, 0)` if unset).
    pub fn counter_pair(&self, key: CardId) -> (i32, i32) {
        self.card_counters.get(&key).copied().unwrap_or_default()
    }

    /// Add `(a, b)` to the two-part card counter `key`.
    pub fn add_counter_pair(&mut self, key: CardId, (a, b): (i32, i32)) {
        let counter = self.card_counters.entry(key).or_default();
        counter.0 += a;
        counter.1 += b;
    }

    /// Record `stacks` of the player effect of `card_id`, lasting `duration` (merged into an
    /// existing effect of the same card and duration).
    pub fn add_effect(&mut self, card_id: CardId, stacks: u32, duration: EffectDuration) {
        if stacks == 0 {
            return;
        }
        match self
            .effects
            .iter_mut()
            .find(|e| e.card_id == card_id && e.duration == duration)
        {
            Some(effect) => effect.stacks += stacks,
            None => self.effects.push(PlayerEffect {
                card_id,
                stacks,
                duration,
            }),
        }
    }

    /// Total stacks of the player effects of `card_id`.
    pub fn effect_stacks(&self, card_id: CardId) -> u32 {
        self.effects
            .iter()
            .filter(|e| e.card_id == card_id)
            .map(|e| e.stacks)
            .sum()
    }

    /// End the player effects that last until the start of the turn.
    pub fn expire_turn_effects(&mut self) {
        self.effects.retain(|e| e.duration != EffectDuration::Turn);
    }

    /// A shop Refresh happened: count down the player effects that last a number of Refreshes,
    /// ending those with none left.
    pub fn count_down_refresh_effects(&mut self) {
        for effect in &mut self.effects {
            if let EffectDuration::Refreshes(n) = &mut effect.duration {
                *n = n.saturating_sub(1);
            }
        }
        let used_up = EffectDuration::Refreshes(0);
        self.effects.retain(|e| e.duration != used_up);
    }
}

/// A minion on a board, in hand, or in Bob's shop, or a spell card in shop/hand.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unit {
    pub id: UnitId,
    pub card_id: CardId,
    pub name: String,
    pub attack: i32,
    pub health: i32,
    /// Maximum Attack and Health reached by this unit (used by `Y'Shaarj` resummon).
    pub max_attack: i32,
    pub max_health: i32,
    pub base_attack: i32,
    pub base_health: i32,
    pub tavern_tier: u32,
    pub tribe: Tribe,
    pub taunt: bool,
    pub divine_shield: bool,
    /// True if Divine Shield is printed on the card (so Reborn restores it).
    pub inherent_divine_shield: bool,
    pub windfury: bool,
    pub reborn: bool,
    pub venomous: bool,
    pub stealth: bool,
    pub magnetic: bool,
    pub is_golden: bool,
    /// True if this card is intrinsically Golden in the shop (`Aureate Laureate`).
    pub intrinsic_golden: bool,
    /// True if this unit is an awakened Deity (`C'Thun` or `Y'Shaarj`).
    pub is_deity: bool,
    /// True if this card is a spell in shop/hand rather than a minion.
    pub is_spell: bool,
    /// Gold (or Health) cost to buy this spell from the shop.
    pub spell_cost: u32,
    /// True if buying this card from the shop costs Health instead of Gold (`Hasty Excavation`).
    pub costs_health: bool,
    /// True if this hand card cannot be manually played (`Lockbox`).
    pub unplayable: bool,
    /// Turns remaining until this `Lockbox` opens (`Bilgewater Breakout`).
    pub lockbox_turns_left: u32,
    /// Turns remaining that this card is locked in hand (`Search Through Time`).
    pub locked_turns: u32,
    /// Group ID and remaining plays for cards generated by `Wandering Willbreaker` / `Faceless Operative`.
    pub willbreaker_group: u32,
    pub willbreaker_remaining: u32,
    /// Stacks of `Winner's Bread` pending if the player wins their next combat.
    pub winners_bread_stacks: u32,
    /// Stats currently applied to this unit by "wherever this is" auras (player-wide effects and
    /// its card's `aura_bonus`), so they can be re-synced when the auras change.
    pub aura_applied: (i32, i32),
    /// Uses left of a limited ability of this unit's card ("once per turn", "3 times per
    /// combat", ...) or uses it has queued; reset by the card's `reset_turn_charges` /
    /// `combat_start` hooks.
    pub charges: u32,
    /// Progress of this unit's card toward a repeating threshold ("after you spend 5 Gold",
    /// "improves each turn", ...). A triple keeps the copies' highest.
    pub counter: i32,
    /// Permanent improvements of this unit's card ("improve this"). A triple keeps the copies'
    /// sum; improvements gained in combat persist.
    pub stacks: i32,
    /// Effects this unit's card queued for its `resolve_pending` hook.
    pub pending: u32,
    /// True if this unit cannot gain positive stats (`Fishbait`).
    pub cant_gain_stats: bool,
    /// True if this unit has triggered its once-per-game stat threshold (`Scarlet Survivor`, `Treasure Parrot`)
    /// or its first-time Volumizer play/magnetize effect.
    pub threshold_triggered: bool,
    /// True if this unit's `Activate` ability has already been used this Tavern turn.
    pub activated_this_turn: bool,
    /// Extra end-of-turn Health bonus transferred from magnetized `Lullabot`s.
    pub eot_health_bonus: i32,
    /// Extra start-of-turn Gold bonus transferred from magnetized `Accord-o-Tron`s.
    pub sot_gold_bonus: u32,
    /// Friendly death counter for `Avenge (X)` effects in combat (`Relentless Deflector`).
    pub avenge_counter: u32,
    /// Permanent Attack/Health gained during combat (`Devout Hellcaller`, `Razorfen Vineweaver`, `Ship Master Eudora`).
    pub perm_atk_gained: i32,
    pub perm_hp_gained: i32,
    /// Permanent Blood Gem count gained during combat (`Razorfen Vineweaver`).
    pub perm_blood_gems_gained: u32,
    /// Total Blood Gems played on this unit and total `(atk, hp)` granted by them (`Gem Confiscation`).
    pub blood_gems_played: u32,
    pub blood_gem_stats_applied: (i32, i32),
    /// True if this hand minion dies immediately if played this turn (`Tomb Turning`).
    pub dies_on_play_this_turn: bool,
    /// True if choosing this Discover option deals damage equal to its Tier (`Imposing Percussionist`).
    pub discover_deals_tier_damage: bool,
    /// True if this unit's Divine Shield is temporary until next turn (`Ichoron the Protector`).
    pub temp_divine_shield: bool,
    /// Board-aura bonus to Tavern spell Attack/Health contributed while this unit is on the board (`Enchanted Sentinel`, `Humon'gozz`).
    pub spell_atk_aura: i32,
    pub spell_hp_aura: i32,
    /// Combat `UnitId` of the minion that dealt the killing blow to this unit (`Leeroy the Reckless`).
    pub killed_by: Option<UnitId>,
    /// Tavern spell taught to a `Magicfin Apprentice` token (`Magicfin Mycologist`).
    pub taught_spell_id: Option<CardId>,
    /// Total Magnetizations attached to this unit (`Utility Drone`).
    pub magnetizations_count: u32,
    /// True if this Choose One card has both effects combined (`Fandral's Fortune`).
    pub combine_choose_one: bool,
    /// Minions destroyed at Start of Combat and stored inside `Stitched Salvager` for its Deathrattle.
    pub stitched_stored: Vec<Unit>,
}

impl Unit {
    /// Construct a vanilla minion (Tier 1, no keywords).
    pub fn new(name: impl Into<String>, attack: i32, health: i32) -> Self {
        Self {
            id: 0,
            card_id: 0,
            name: name.into(),
            attack,
            health,
            max_attack: attack,
            max_health: health,
            base_attack: attack,
            base_health: health,
            tavern_tier: 1,
            tribe: Tribe::None,
            taunt: false,
            divine_shield: false,
            inherent_divine_shield: false,
            windfury: false,
            reborn: false,
            venomous: false,
            stealth: false,
            magnetic: false,
            is_golden: false,
            intrinsic_golden: false,
            is_deity: false,
            is_spell: false,
            spell_cost: 0,
            costs_health: false,
            unplayable: false,
            lockbox_turns_left: 0,
            locked_turns: 0,
            willbreaker_group: 0,
            willbreaker_remaining: 0,
            winners_bread_stacks: 0,
            aura_applied: (0, 0),
            charges: 0,
            counter: 0,
            stacks: 0,
            pending: 0,
            cant_gain_stats: false,
            threshold_triggered: false,
            activated_this_turn: false,
            eot_health_bonus: 0,
            sot_gold_bonus: 0,
            avenge_counter: 0,
            perm_atk_gained: 0,
            perm_hp_gained: 0,
            perm_blood_gems_gained: 0,
            blood_gems_played: 0,
            blood_gem_stats_applied: (0, 0),
            dies_on_play_this_turn: false,
            discover_deals_tier_damage: false,
            temp_divine_shield: false,
            spell_atk_aura: 0,
            spell_hp_aura: 0,
            killed_by: None,
            taught_spell_id: None,
            magnetizations_count: 0,
            combine_choose_one: false,
            stitched_stored: Vec::new(),
        }
    }

    pub fn with_card_id(mut self, card_id: CardId) -> Self {
        self.card_id = card_id;
        self
    }

    pub fn with_tavern_tier(mut self, tier: u32) -> Self {
        self.tavern_tier = tier;
        self
    }

    pub fn with_tribe(mut self, tribe: Tribe) -> Self {
        self.tribe = tribe;
        self
    }

    pub fn with_keyword(mut self, kw: Keyword) -> Self {
        self.apply_keyword(kw, true);
        self
    }

    pub fn with_golden(mut self, is_golden: bool) -> Self {
        self.is_golden = is_golden;
        self
    }

    /// Grant a keyword to this unit. If `inherent` is true and the keyword is `DivineShield`,
    /// also sets `inherent_divine_shield = true`.
    pub fn apply_keyword(&mut self, kw: Keyword, inherent: bool) {
        match kw {
            Keyword::Taunt => self.taunt = true,
            Keyword::DivineShield => {
                self.divine_shield = true;
                if inherent {
                    self.inherent_divine_shield = true;
                }
            }
            Keyword::Windfury => self.windfury = true,
            Keyword::Reborn => self.reborn = true,
            Keyword::Venomous => self.venomous = true,
            Keyword::Stealth => self.stealth = true,
            Keyword::Magnetic => self.magnetic = true,
        }
    }

    /// Returns `true` if the unit already has the given keyword.
    pub fn has_keyword(&self, kw: Keyword) -> bool {
        match kw {
            Keyword::Taunt => self.taunt,
            Keyword::DivineShield => self.divine_shield,
            Keyword::Windfury => self.windfury,
            Keyword::Reborn => self.reborn,
            Keyword::Venomous => self.venomous,
            Keyword::Stealth => self.stealth,
            Keyword::Magnetic => self.magnetic,
        }
    }

    /// Buff this unit's Attack and Health, updating `max_attack` / `max_health` and
    /// running stat-threshold checks. Respects `cant_gain_stats` (`Fishbait`).
    pub fn add_stats(&mut self, atk: i32, hp: i32) {
        let eff_atk = if self.cant_gain_stats && atk > 0 {
            0
        } else {
            atk
        };
        let eff_hp = if self.cant_gain_stats && hp > 0 {
            0
        } else {
            hp
        };
        self.attack += eff_atk;
        self.health += eff_hp;
        if self.attack > self.max_attack {
            self.max_attack = self.attack;
        }
        if self.health > self.max_health {
            self.max_health = self.health;
        }
        crate::cards::check_stat_thresholds(self);
    }

    /// Play `count` Blood Gem(s) on this unit using the player's current `auras`.
    pub fn play_blood_gems(&mut self, count: u32, auras: &PlayerAuras) {
        if count == 0 {
            return;
        }
        let (g_atk, g_hp) = auras.blood_gem_stats();
        let c = count as i32;
        let d_atk = g_atk * c;
        let d_hp = g_hp * c;
        self.blood_gems_played += count;
        self.blood_gem_stats_applied.0 += d_atk;
        self.blood_gem_stats_applied.1 += d_hp;
        self.add_stats(d_atk, d_hp);
        crate::cards::on_blood_gems_played_on_unit(self, count);
    }

    /// Convert this plain minion into a Golden version (`Golden Touch`, `Elite Navigator`).
    /// Aura bonuses that scale with Golden are re-applied by the next aura sync
    /// ([`crate::cards::sync_unit_auras`]).
    pub fn make_golden(&mut self) {
        if self.is_golden {
            return;
        }
        let add_atk = self.base_attack.max(0);
        let add_hp = self.base_health.max(0);
        self.base_attack *= 2;
        self.base_health *= 2;
        self.add_stats(add_atk, add_hp);
        self.is_golden = true;
        self.intrinsic_golden = true;
        if !self.name.starts_with("Golden ") {
            self.name = format!("Golden {}", self.name);
        }
        let card = crate::cards::hooks(self.card_id);
        let (aura_atk, aura_hp) = card.spell_aura;
        self.spell_atk_aura += aura_atk;
        self.spell_hp_aura += aura_hp;
        if let Some(made_golden) = card.made_golden {
            made_golden(self);
        }
    }

    /// Refresh `max_attack` and `max_health` from current stats.
    pub fn sync_max_stats(&mut self) {
        if self.attack > self.max_attack {
            self.max_attack = self.attack;
        }
        if self.health > self.max_health {
            self.max_health = self.health;
        }
    }

    /// The usual Golden effect multiplier: `2` if this unit is Golden, else `1`.
    pub fn golden_mult(&self) -> i32 {
        if self.is_golden {
            2
        } else {
            1
        }
    }

    /// Count one friendly death towards this unit's `Avenge (n)`. Returns `true` (and restarts
    /// the count) when the Avenge triggers.
    pub fn avenge(&mut self, n: u32) -> bool {
        self.avenge_counter += 1;
        if self.avenge_counter >= n {
            self.avenge_counter -= n;
            true
        } else {
            false
        }
    }
}

/// Which side of a battle a unit or hero belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    A,
    B,
}

impl Side {
    #[inline]
    pub fn other(self) -> Side {
        match self {
            Side::A => Side::B,
            Side::B => Side::A,
        }
    }
}

/// Outcome of a single battle (`docs/combat.md` §5.6).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleOutcome {
    AWin,
    BWin,
    Draw,
}

/// Pre-battle game state for both sides (hero tiers, persistent player auras/deities, and hands).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameState {
    pub hero_tier_a: u32,
    pub hero_tier_b: u32,
    pub auras_a: PlayerAuras,
    pub auras_b: PlayerAuras,
    pub hand_a: Vec<Unit>,
    pub hand_b: Vec<Unit>,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            hero_tier_a: 1,
            hero_tier_b: 1,
            auras_a: PlayerAuras::default(),
            auras_b: PlayerAuras::default(),
            hand_a: Vec::new(),
            hand_b: Vec::new(),
        }
    }
}
