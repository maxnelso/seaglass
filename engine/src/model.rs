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

/// Minion tribe classification (Patch 36.6.3), including dual-tribe combinations: the 10 lobby
/// types ([`LOBBY_TRIBES`]) and `Naga` (`docs/tavern.md` §7.8). `Naga` and its dual types come
/// last, so the earlier variants keep their discriminants.
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
    /// Not a lobby type in Patch 36.6.3: the pool's Nagas have a lobby type too (`DragonNaga`,
    /// `DemonNaga`).
    Naga,
    DragonNaga,
    DemonNaga,
}

/// The 10 lobby types of Patch 36.6.3: every base single minion tribe but `Naga`, which no lobby
/// has.
pub const LOBBY_TRIBES: [Tribe; 10] = [
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

/// All 11 base single minion tribes in Patch 36.6.3: the 10 lobby types, then `Naga`, which no
/// lobby has but some minions carry as their second type (`docs/tavern.md` §7.8). Every effect
/// that looks at minion types counts all of them.
pub const SINGLE_TRIBES: [Tribe; 11] = [
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
    Tribe::Naga,
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
                    | Tribe::Naga
            ),
            Tribe::UndeadMech => matches!(single, Tribe::Undead | Tribe::Mech),
            Tribe::DragonPirate => matches!(single, Tribe::Dragon | Tribe::Pirate),
            Tribe::BeastPirate => matches!(single, Tribe::Beast | Tribe::Pirate),
            Tribe::ElementalDemon => matches!(single, Tribe::Elemental | Tribe::Demon),
            Tribe::MechMurloc => matches!(single, Tribe::Mech | Tribe::Murloc),
            Tribe::DemonDragon => matches!(single, Tribe::Demon | Tribe::Dragon),
            Tribe::DemonQuilboar => matches!(single, Tribe::Demon | Tribe::Quilboar),
            Tribe::DragonNaga => matches!(single, Tribe::Dragon | Tribe::Naga),
            Tribe::DemonNaga => matches!(single, Tribe::Demon | Tribe::Naga),
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
            Tribe::Naga => "naga",
            Tribe::DragonNaga => "dragon_naga",
            Tribe::DemonNaga => "demon_naga",
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
    /// Bonus Health applied to friendly Undead (`Butchering`'s Health spell power).
    pub undead_bonus_health: i32,
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
    /// Tavern spells cast this game (`Faceless Converter`): every cast of a spell for which
    /// [`crate::cards::spells::is_tavern_spell`] holds, so not Blood Gems (`docs/tavern.md` §7.4).
    pub tavern_spells_cast: u32,
    /// Cost reduction on the next Tavern spell bought (`Ominous Seer`).
    pub next_spell_discount: i32,
    /// Cost reduction on the next minion bought (Aranna Starseeker; `docs/tavern.md` §6): the
    /// purchase uses it up.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_minion_discount: i32,
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
    /// Duos: the team's Passes this turn and this game, either teammate's, mirrored into both
    /// teammates' auras by the team's Tavern (`docs/duos_tavern.md` §7). Always 0 in Solo.
    pub team_passes_this_turn: u32,
    pub team_passes_this_game: u32,
    /// Duos: the player is in a Duos game, so their random-minion effects draw from the Duos
    /// catalog (`docs/duos_tavern.md` §8). Set by the team's Tavern and carried into combat by
    /// the warband; `false` in Solo.
    #[serde(default)]
    pub duos_game: bool,
    /// Duos: the teammate's `card_counters`, mirrored by the team's Tavern, so that a "your
    /// team's" count is [`Self::team_counter`] (`docs/duos_tavern.md` §7.1). Empty in Solo.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub teammate_counters: BTreeMap<CardId, (i32, i32)>,
    /// Duos: what this player's cards gave their teammate, in order, waiting for the team's
    /// Tavern to deliver it (`docs/duos_tavern.md` §7.2), or in a Duos combat for the team's
    /// next delivery there (`docs/duos.md` §2). Nothing delivers it in Solo.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub to_teammate: Vec<TeammateGift>,
    /// The player's hero and its Hero Power's state (`docs/heroes.md` §2.2), `None` for a player
    /// without a hero. It lives in the auras so that it travels into combat and back with them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hero: Option<HeroState>,
    /// The player's trinkets, in the order they were picked (`docs/trinkets.md` §2.2): empty for a
    /// player without trinkets. They live in the auras so that they travel into combat and back.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trinkets: Vec<TrinketState>,
}

/// A trinket the player owns (`docs/trinkets.md` §2.2). The engine reads only `card_id`; the rest
/// belongs to the trinket's own hooks.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrinketState {
    /// The trinket (its dbfId, the card whose hooks implement it).
    pub card_id: CardId,
    /// Progress toward a threshold ("(6 left!)").
    #[serde(default)]
    pub counter: i32,
    /// Improvements so far.
    #[serde(default)]
    pub stacks: i32,
    /// A per-turn or per-combat charge.
    #[serde(default)]
    pub armed: u32,
    /// Cards the trinket keeps.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stored: Vec<Unit>,
}

/// A trinket offered to the player (`docs/trinkets.md` §3.3): which one, and its price.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrinketOption {
    pub card_id: CardId,
    /// Its price in Gold (2 less than its cost for an option of a type that isn't the player's
    /// most common one).
    pub cost: u32,
}

/// A hero's identity: the hero card's dbfId (`64403` for `Al'Akir`; `docs/heroes.md` §2.1).
pub type HeroId = u32;

/// A player's hero (`docs/heroes.md` §2.2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroState {
    pub hero_id: HeroId,
    /// Its Hero Power's state.
    pub power: HeroPowerState,
    /// A second Hero Power (`Genn, Worgen King`'s King of Duality; `docs/heroes.md` §2.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second_power: Option<HeroPowerState>,
}

/// The state of a player's Hero Power (`docs/heroes.md` §3.2). The engine reads only `card_id`,
/// `cost` and `uses_left`; the rest belongs to the Hero Power's own hooks.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroPowerState {
    /// The Hero Power card in use (its hooks): the hero's power, or a form it changed into (a
    /// chosen Element, an upgrade).
    pub card_id: CardId,
    /// Its current Gold cost.
    pub cost: u32,
    /// Uses left this turn (reset at the start of each turn).
    pub uses_left: u32,
    /// Progress toward a threshold ("14 left!", Gold spent toward the next improvement).
    #[serde(default)]
    pub counter: i32,
    /// Improvements so far.
    #[serde(default)]
    pub stacks: i32,
    /// Next-combat effects paid for, used up by the player's next combat.
    #[serde(default)]
    pub armed: u32,
    /// Cards the Hero Power keeps (a copy waiting for room in combat).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stored: Vec<Unit>,
    /// A minion on the board or in the shop the Hero Power picked and will act on again (a first
    /// pick waiting for its second), by the id `heroes::identify_minion` gave it. It drops once no
    /// minion on the board or in the shop has that id (`docs/heroes.md` §3.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picked: Option<UnitId>,
}

/// Something a player's card gives their teammate (`docs/duos_tavern.md` §7.2), recorded in
/// [`PlayerAuras::to_teammate`] and delivered by the team's Tavern, or in a Duos combat by the
/// combat, but for what needs the Tavern (`docs/duos.md` §2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeammateGift {
    /// The teammate gets this card: it arrives in their hand like a passed card (in a Duos
    /// combat, in their combat hand if it has room).
    Card(Unit),
    /// The player's card effect Passes this card to the teammate: a Pass without its payment.
    Pass(Unit),
    /// The giving card's `teammate_gift` hook runs on the teammate's Tavern with `amount`, or
    /// on both players' Taverns if `team` (the giver's first). In a Duos combat, its
    /// `teammate_combat_gift` hook runs on the teammate's warband instead, if it has one and
    /// the gift is not the team's.
    Effect {
        card_id: CardId,
        amount: i32,
        team: bool,
    },
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

    /// Duos: the team's value of the card counter `key`: this player's plus their teammate's
    /// (`docs/duos_tavern.md` §7.1). In Solo it is the player's own.
    pub fn team_counter(&self, key: CardId) -> i32 {
        self.counter(key) + self.teammate_counters.get(&key).map_or(0, |c| c.0)
    }

    /// Duos: give the teammate `gift` (`docs/duos_tavern.md` §7.2).
    pub fn give_teammate(&mut self, gift: TeammateGift) {
        self.to_teammate.push(gift);
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
    /// Taunt an effect gave it (a spell, a Magnetization, ...: `apply_keyword(Taunt)` from outside
    /// its card), which a conditional Taunt doesn't take away: `Relentless Deflector` keeps it
    /// whatever its Divine Shield (`docs/tavern.md` §7.4).
    #[serde(default, skip_serializing_if = "is_false")]
    pub granted_taunt: bool,
    pub divine_shield: bool,
    /// True if Divine Shield is printed on the card (so Reborn restores it).
    pub inherent_divine_shield: bool,
    pub windfury: bool,
    pub reborn: bool,
    pub venomous: bool,
    pub stealth: bool,
    pub magnetic: bool,
    pub is_golden: bool,
    /// True if this Golden doesn't give a Triple Reward when played (and counts as a single copy
    /// for the card pool): it is intrinsically Golden (`Aureate Laureate`) or the card that made
    /// it says so (`Silent Deliverer`). Other Goldens made by effects are regular Goldens.
    pub intrinsic_golden: bool,
    /// True if an effect made this Golden from a single copy (`Golden Touch`, `Captain Sanders`,
    /// ...; see [`Self::make_golden`]) rather than a Triple: it still gives a Triple Reward, but
    /// counts as a single copy for the card pool (`docs/tavern.md` §2).
    pub golden_from_effect: bool,
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
    /// Duos: Attack and Health this unit has for each time its team Passed this game, wherever
    /// it is, from the `Transport Reactor`s Magnetized onto it (`docs/duos_tavern.md` §8.2).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pass_aura: i32,
    /// Duos: this Discover option goes to the teammate if another option is chosen, instead of
    /// back to the pool (`Bargain Bundle`, `docs/duos_tavern.md` §8.3).
    #[serde(default, skip_serializing_if = "is_false")]
    pub unchosen_to_teammate: bool,
    /// Marked by its player's Hero Power, which acts on it at the next Start of Combat (Teron's
    /// Rapid Reanimation, Floop's Glorious Gloop; `docs/heroes.md` §3.3).
    #[serde(default, skip_serializing_if = "is_false")]
    pub marked: bool,
    /// Deathrattles an effect gave it in combat (Tamsin's Fragrant Phylactery, Bru'kan's Earth
    /// Invocation): the `deathrattle` hooks of these cards resolve after its own
    /// (`docs/heroes.md` §4).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_deathrattles: Vec<CardId>,
    /// True if this unit's Reborn is temporary until next turn (The Lich King's Reborn Rites;
    /// `docs/heroes.md` §11), as `temp_divine_shield` for Divine Shield.
    #[serde(default, skip_serializing_if = "is_false")]
    pub temp_reborn: bool,
    /// True if this card in the shop is frozen on its own (Varden Dawngrasp's copies;
    /// `docs/tavern.md` §3.1): it stays through the next turn start.
    #[serde(default, skip_serializing_if = "is_false")]
    pub frozen: bool,
    /// A Discover option a Hero Power keeps (`docs/tavern.md` §5.2): chosen, it goes to the Hero
    /// Power's `stored` cards instead of the hand (Thorim, Stormlord).
    #[serde(default, skip_serializing_if = "is_false")]
    pub to_hero_power: bool,
    /// The copies a Triple took to form this Golden, if not 3 (Mister Clocksworth's 2;
    /// `docs/tavern.md` §5): selling it returns as many to the pool. 0 for 3.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub golden_copies: u8,
    /// Attack gained "until next turn" (`Vol'jin`'s Spirit Swap; `docs/heroes.md` §11.2): expires
    /// at the start of the next turn (`docs/tavern.md` §4).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub until_next_turn_atk: i32,
    /// Number of +1/+1 hats on this minion: copies of the hat card [`HAT_CARD`] (`Dancin' Deryl`'s
    /// Hat Trick gives them; `docs/heroes.md` §11.2). When it is sold, the hat card's `sell` hook
    /// passes them on, whatever the player's Hero Power (`docs/tavern.md` §7.3).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub hats: u32,
}

/// The hat card (*Hats*, `TB_BaconShop_HP_042e`): [`Unit::hats`] counts its copies on a minion,
/// and its `sell` hook passes them when the minion is sold (`docs/heroes.md` §11.2).
pub const HAT_CARD: CardId = 59861;

/// `true` if `n` is 0 (serde: a field skipped while it is 0).
fn is_zero_u8(n: &u8) -> bool {
    *n == 0
}

/// `true` if `n` is 0 (serde: a field skipped while it is 0).
fn is_zero_u32(n: &u32) -> bool {
    *n == 0
}

/// `true` if `n` is 0 (serde: a field skipped while it is 0).
fn is_zero(n: &i32) -> bool {
    *n == 0
}

/// `true` if `b` is `false` (serde: a field skipped while it is `false`).
fn is_false(b: &bool) -> bool {
    !*b
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
            granted_taunt: false,
            divine_shield: false,
            inherent_divine_shield: false,
            windfury: false,
            reborn: false,
            venomous: false,
            stealth: false,
            magnetic: false,
            is_golden: false,
            intrinsic_golden: false,
            golden_from_effect: false,
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
            pass_aura: 0,
            unchosen_to_teammate: false,
            marked: false,
            extra_deathrattles: Vec::new(),
            temp_reborn: false,
            frozen: false,
            to_hero_power: false,
            golden_copies: 0,
            until_next_turn_atk: 0,
            hats: 0,
        }
    }

    /// The copies of its card this unit holds for the card pool (`docs/tavern.md` §2): 3 for a
    /// Golden a Triple formed (or its `golden_copies`), 1 for any other minion (a Golden an effect
    /// made from a single copy included), 0 for a spell.
    pub fn pool_copies(&self) -> u32 {
        if self.is_spell {
            0
        } else if self.is_golden && !self.intrinsic_golden && !self.golden_from_effect {
            match self.golden_copies {
                0 => 3,
                n => u32::from(n),
            }
        } else {
            1
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
            Keyword::Taunt => {
                self.taunt = true;
                if !inherent {
                    self.granted_taunt = true;
                }
            }
            Keyword::DivineShield => {
                self.divine_shield = true;
                // A lasting Divine Shield over one until next turn makes it lasting.
                self.temp_divine_shield = false;
                if inherent {
                    self.inherent_divine_shield = true;
                }
            }
            Keyword::Windfury => self.windfury = true,
            Keyword::Reborn => {
                self.reborn = true;
                // A lasting Reborn over one until next turn makes it lasting.
                self.temp_reborn = false;
            }
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

    /// Give this unit `atk` Attack until next turn (`Vol'jin`'s Spirit Swap).
    pub fn add_temp_attack(&mut self, atk: i32) {
        if atk <= 0 || self.cant_gain_stats {
            return;
        }
        self.until_next_turn_atk += atk;
        self.add_stats(atk, 0);
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
    /// Like a tripled Golden, it gives a Triple Reward when played (unless the caller sets
    /// [`Self::intrinsic_golden`]), but it counts as a single copy for the card pool
    /// ([`Self::golden_from_effect`]). Aura bonuses that scale with Golden are re-applied by the
    /// next aura sync ([`crate::cards::sync_unit_auras`]).
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
        self.golden_from_effect = true;
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

fn default_true() -> bool {
    true
}

/// Caller-supplied snapshot of one opponent in the lobby (`docs/tavern.md` §3.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpponentView {
    #[serde(default = "default_true")]
    pub alive: bool,
    #[serde(default)]
    pub died_last_combat: bool,
    #[serde(default)]
    pub last_warband: Vec<Unit>,
}

impl Default for OpponentView {
    fn default() -> Self {
        Self {
            alive: true,
            died_last_combat: false,
            last_warband: Vec::new(),
        }
    }
}

/// Caller-supplied snapshot of the other players in the lobby (`docs/tavern.md` §3.1).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LobbyView {
    #[serde(default)]
    pub opponents: Vec<OpponentView>,
    #[serde(default)]
    pub next_opponent: Option<usize>,
    #[serde(default)]
    pub last_opponent: Option<usize>,
    #[serde(default)]
    pub featured_pair: Option<(usize, usize)>,
    #[serde(default)]
    pub last_pair_outcome: Option<BattleOutcome>,
}

impl LobbyView {
    pub fn is_empty(&self) -> bool {
        self.opponents.is_empty()
            && self.next_opponent.is_none()
            && self.last_opponent.is_none()
            && self.featured_pair.is_none()
            && self.last_pair_outcome.is_none()
    }
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
