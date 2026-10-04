//! Core data types: units, tribes, keywords, deities, player auras, and game state.

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

/// Persistent game-long scaling counters and auras (`docs/tavern.md` §7.6).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerAuras {
    /// Bonus Attack/Health applied to Elementals in the Tavern (`Dune Dweller`).
    pub tavern_elemental_atk: i32,
    pub tavern_elemental_hp: i32,
    /// Bonus Attack/Health applied to all minions in the Tavern (`Staff of Enrichment`).
    pub tavern_all_atk: i32,
    pub tavern_all_hp: i32,
    /// Turn-scoped bonus Attack/Health applied to all minions in the Tavern (`Ashen Corruptor`).
    pub ashen_corruptor_turn_buff: i32,
    /// Bonus Attack/Health applied to summoned Beetles (`Buzzing Vermin` / `Forest Rover`).
    pub beetle_bonus_atk: i32,
    pub beetle_bonus_hp: i32,
    /// Bonus Attack applied to friendly Undead (`Nerubian Deathswarmer`).
    pub undead_bonus_attack: i32,
    /// Bonus Attack/Health added to Blood Gems (`Gem Day` / `Crater Miner` / `Fearless Foodie`).
    pub blood_gem_bonus_atk: i32,
    pub blood_gem_bonus_hp: i32,
    /// Extra Attack/Health granted by Tavern spells that give stats (`Intrepid Botanist`, `Azsharan Cutlassier`, `Blue Whelp`, `Shoalfin Mystic`).
    pub spell_bonus_atk: i32,
    pub spell_bonus_hp: i32,
    /// Bonus Attack/Health applied to Volumizers wherever they are (`Red`/`Blue`/`Green Volumizer`).
    pub volumizer_bonus_atk: i32,
    pub volumizer_bonus_hp: i32,
    /// Bonus stats added to future `Fire Baller` and `Snow Baller` sell triggers.
    pub baller_bonus: i32,
    /// Number of `(+2, +1)` improvement stacks added to future `Tasty Lobster` Deathrattles.
    pub tasty_lobster_stacks: i32,
    /// Number of friendly `Eternal Knight`s that have died this game.
    pub eternal_knights_died: u32,
    /// Number of `Waveling` Deathrattle stacks (each gives a random Tavern minion `+4/+4` on Refresh).
    pub waveling_stacks: u32,
    /// Number of `Demon Fodder`s to add on the next 3 shop Refreshes (`Laboratory Assistant` / `Trapped Clapper`).
    pub fodder_per_refresh: [u32; 3],
    /// Free shop Refreshes remaining (`Leaf Through the Pages` / `Sly Infiltrator`).
    pub free_refreshes: u32,
    /// Permanent bonus to maximum Gold (`Strike Oil`).
    pub base_max_gold_bonus: u32,
    /// Stacks of `Overconfidence` active for the next combat (`+3` Gold on win, `+1` on tie).
    pub overconfidence_stacks: u32,
    /// Number of `+2/+2` board buffs queued for the start of next turn (`Time Management`).
    pub time_management_next_turn: u32,
    /// Total spells cast this game.
    pub spells_played: u32,
    /// Cost reduction on the next Tavern spell bought (`Ominous Seer`).
    pub next_spell_discount: i32,
    /// Per-tribe Tavern shop buffs (`Eonar's Favor`).
    pub tavern_tribe_buffs: Vec<(Tribe, i32, i32)>,
    /// Random single-minion Tavern shop buffs triggered on each `Refresh` (`Easterly Winds`, `En-Djinn Blazer`).
    pub refresh_random_buffs: Vec<(i32, i32)>,
    /// Stacks of `Blood Gem Barrage` (each stack plays 2 Blood Gems on every Tavern minion on `Refresh`).
    pub blood_gem_barrage_stacks: u32,
    /// Remaining Taunt `Beetle` summons when space opens in combat (`Boon of Beetles`).
    pub boon_of_beetles_charges: u32,
    /// Total cards discarded from hand this game (`Parasitic Fleshling`).
    pub cards_discarded: u32,
    /// Total Golden minions played this game (`Maritime Extortionist`).
    pub golden_minions_played: u32,
    /// True if the player's hero currently has 15 or less Health (`Holy Vanguard`).
    pub hero_low_health: bool,
    /// Currently applied board-aura contribution to `(spell_bonus_atk, spell_bonus_hp)` (`Enchanted Sentinel`, `Humon'gozz`).
    pub board_spell_bonus_applied: (i32, i32),
    /// Card ID of the last Tavern spell cast (`Cataclysmic Harbinger`).
    pub last_tavern_spell_cast: Option<CardId>,
    /// Temporary `+X/+X` bonus to friendly Beasts until next turn (`Goldrinn, the Great Wolf`).
    pub goldrinn_bonus: i32,
    /// Queued Start-of-Combat left-most Attack doublings (`Brood of Nozdormu`).
    pub brood_of_nozdormu_stacks: u32,
    /// Queued Start-of-Combat random enemy Health-to-1 triggers (`Upper Hand`).
    pub upper_hand_stacks: u32,
    /// Discovered Hero Power ID (`Unmasked Identity`).
    pub hero_power_id: u32,
    /// Total Deathrattles triggered this game (`Falling Sky Golem`).
    pub deathrattles_triggered: u32,
    /// Queued Start-of-Combat left-most stat gains from the nearest enemy minion (`Sharing is Caring`).
    pub sharing_is_caring_stacks: u32,
    /// Player's Old God Deity state (awakens after 4 friendly Aberration deaths in combat).
    pub deity: DeityState,
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
}

/// A minion on a board, in hand, or in Bob's shop, or a spell card in shop/hand.
#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Current Discover tier for `Patient Scout` (`1..=6`, upgrades each turn on board).
    pub scout_tier: u32,
    /// Group ID and remaining plays for cards generated by `Wandering Willbreaker` / `Faceless Operative`.
    pub willbreaker_group: u32,
    pub willbreaker_remaining: u32,
    /// Stacks of `Winner's Bread` pending if the player wins their next combat.
    pub winners_bread_stacks: u32,
    /// Stats currently applied to this unit by "wherever this is" auras (player-wide effects and
    /// its card's `aura_bonus`), so they can be re-synced when the auras change.
    pub aura_applied: (i32, i32),
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
    /// Remaining Refreshes this turn that cost Health instead of Gold (`Malchezaar, Prince of Dance`).
    pub malchezaar_refreshes_left: u32,
    /// Remaining Choose-One combined charges this turn (`Thorned Trailblazer`).
    pub trailblazer_charges_left: u32,
    /// Friendly death counter for `Avenge (X)` effects in combat (`Relentless Deflector`).
    pub avenge_counter: u32,
    /// Cumulative damage dealt by this unit (`Treasure Parrot`).
    pub damage_dealt_counter: i32,
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
    /// Extra Magnetize repetitions queued for the next Magnetization onto this unit this turn (`Drone Duplicator`).
    pub extra_magnetize_this_turn: u32,
    /// Multiplier queued for the next minion bought this turn (`Living Prison`).
    pub living_prison_stacks: u32,
    /// Gold-spent progress toward the next Gold-spent trigger (`Gunpowder Courier`, `Air Revenant`, `Enterprising Escapee`, `Sky Admiral Rogers`).
    pub gunpowder_gold_progress: u32,
    /// Bonus Tavern-all stats added to `Sacrificial Wrathguard`'s Deathrattle via `Activate`.
    pub wrathguard_bonus: i32,
    /// Queued `Geomagus Roogug` Blood Gem procs to distribute to another friendly minion.
    pub pending_roogug_gems: u32,
    /// Board-aura bonus to Tavern spell Attack/Health contributed while this unit is on the board (`Enchanted Sentinel`, `Humon'gozz`).
    pub spell_atk_aura: i32,
    pub spell_hp_aura: i32,
    /// Spells-cast progress toward the next 3-spell Tavern consume (`Felboar`).
    pub felboar_spell_progress: u32,
    /// Permanent improvement stacks from friendly Divine Shield losses (`Hopebringer`).
    pub hopebringer_stacks: u32,
    /// Permanent improvement stacks from friendly Beast summons (`Lurking Leviathan`).
    pub leviathan_stacks: u32,
    /// Permanent improvement stacks from friendly Mech plays (`Spark Snapper`).
    pub spark_snapper_stacks: u32,
    /// Combat `UnitId` of the minion that dealt the killing blow to this unit (`Leeroy the Reckless`).
    pub killed_by: Option<UnitId>,
    /// Cards-bought progress toward the next 3-buy Volumizer Magnetization (`Auto Reveille`).
    pub auto_reveille_buys: u32,
    /// Hero damage progress toward the next 4-damage `Corrupted Cupcakes` (`Eredar Escapist`).
    pub eredar_damage_progress: i32,
    /// Permanent improvement stacks from cards discarded (`Harbinger Aph'lass`).
    pub aphlass_stacks: u32,
    /// Remaining `Magicfin Mycologist` spell-buy triggers this turn.
    pub mycologist_charges_left: u32,
    /// Tavern spell taught to a `Magicfin Apprentice` token (`Magicfin Mycologist`).
    pub taught_spell_id: Option<CardId>,
    /// Permanent improvement stacks from Elementals played (`Ultraviolet Ascendant`).
    pub ultraviolet_stacks: u32,
    /// Elementals-played progress toward the next 4-Elemental Tavern stat gain (`Unbound Tempest`).
    pub unbound_tempest_progress: u32,
    /// Total Magnetizations attached to this unit (`Utility Drone`).
    pub magnetizations_count: u32,
    /// True if this Choose One card has both effects combined (`Fandral's Fortune`).
    pub fandral_combined: bool,
    /// Remaining combat summon triggers for `Stalwart Kodo` (`3` per combat).
    pub kodo_triggers_left: u8,
    /// Remaining minion-buy triggers this turn for `Stone Age Slab` (`1` per turn).
    pub slab_charges_left: u8,
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
            scout_tier: 1,
            willbreaker_group: 0,
            willbreaker_remaining: 0,
            winners_bread_stacks: 0,
            aura_applied: (0, 0),
            cant_gain_stats: false,
            threshold_triggered: false,
            activated_this_turn: false,
            eot_health_bonus: 0,
            sot_gold_bonus: 0,
            malchezaar_refreshes_left: 0,
            trailblazer_charges_left: 0,
            avenge_counter: 0,
            damage_dealt_counter: 0,
            perm_atk_gained: 0,
            perm_hp_gained: 0,
            perm_blood_gems_gained: 0,
            blood_gems_played: 0,
            blood_gem_stats_applied: (0, 0),
            dies_on_play_this_turn: false,
            discover_deals_tier_damage: false,
            temp_divine_shield: false,
            extra_magnetize_this_turn: 0,
            living_prison_stacks: 0,
            gunpowder_gold_progress: 0,
            wrathguard_bonus: 0,
            pending_roogug_gems: 0,
            spell_atk_aura: 0,
            spell_hp_aura: 0,
            felboar_spell_progress: 0,
            hopebringer_stacks: 0,
            leviathan_stacks: 0,
            spark_snapper_stacks: 0,
            killed_by: None,
            auto_reveille_buys: 0,
            eredar_damage_progress: 0,
            aphlass_stacks: 0,
            mycologist_charges_left: 0,
            taught_spell_id: None,
            ultraviolet_stacks: 0,
            unbound_tempest_progress: 0,
            magnetizations_count: 0,
            fandral_combined: false,
            kodo_triggers_left: 0,
            slab_charges_left: 0,
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
