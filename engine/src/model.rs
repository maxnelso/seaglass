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

/// Minion tribe classification (Patch 36.6.3).
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
    All,
}

impl Tribe {
    /// Returns `true` if `self` counts as `target` tribe.
    /// `Tribe::None` matches nothing (not even `Tribe::All`).
    pub fn matches(self, target: Tribe) -> bool {
        if self == Tribe::None || target == Tribe::None {
            return false;
        }
        if self == Tribe::All || target == Tribe::All {
            return true;
        }
        self == target
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
    /// Bonus Attack/Health applied to summoned Beetles (`Buzzing Vermin` / `Forest Rover`).
    pub beetle_bonus_atk: i32,
    pub beetle_bonus_hp: i32,
    /// Bonus Attack applied to friendly Undead.
    pub undead_bonus_attack: i32,
    /// Total spells cast this game.
    pub spells_played: u32,
    /// Cost reduction on the next Tavern spell bought (`Ominous Seer`).
    pub next_spell_discount: i32,
    /// Player's Old God Deity state (awakens after 4 friendly Aberration deaths in combat).
    pub deity: DeityState,
}

/// A minion on a board, in hand, or in Bob's shop, or a hand spell card.
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
    /// True if this card is a spell in hand rather than a minion (`Blood Gem`, `Tavern Coin`, etc.).
    pub is_spell: bool,
    /// True if this unit has triggered its once-per-game stat threshold (`Scarlet Survivor`).
    pub threshold_triggered: bool,
    /// True if this unit's `Activate` ability has already been used this Tavern turn.
    pub activated_this_turn: bool,
    /// Extra end-of-turn Health bonus transferred from magnetized `Lullabot`s.
    pub eot_health_bonus: i32,
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
            threshold_triggered: false,
            activated_this_turn: false,
            eot_health_bonus: 0,
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
    /// running stat-threshold checks.
    pub fn add_stats(&mut self, atk: i32, hp: i32) {
        self.attack += atk;
        self.health += hp;
        if self.attack > self.max_attack {
            self.max_attack = self.attack;
        }
        if self.health > self.max_health {
            self.max_health = self.health;
        }
        crate::cards::check_stat_thresholds(self);
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

/// Pre-battle game state for both sides (hero tiers and persistent player auras/deities).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameState {
    pub hero_tier_a: u32,
    pub hero_tier_b: u32,
    pub auras_a: PlayerAuras,
    pub auras_b: PlayerAuras,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            hero_tier_a: 1,
            hero_tier_b: 1,
            auras_a: PlayerAuras::default(),
            auras_b: PlayerAuras::default(),
        }
    }
}
