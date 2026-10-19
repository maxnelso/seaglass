//! Reconstructs every combat of a game as engine input (`engine/docs/duos.md`): both teams'
//! warbands and which one fought first, each player's Tavern Tier, hand and player auras, plus
//! the server's combat event log and the real result.
//!
//! # How the log shows a combat
//!
//! 1. `GameEntity BACON_IN_COMBAT_PHASE` 0 → 1 opens the combat.
//! 2. The server sends a *package* for every other player in the combat (the opponents, and in
//!    Duos the teammate), in `SETASIDE` and controlled by the local player: a `HERO`
//!    (`PLAYER_ID`, `PLAYER_TECH_LEVEL`, `HEALTH`/`DAMAGE`/`ARMOR`; then
//!    `BACON_COMBAT_PHASE_HERO = 1`), its hero power, its `BG_OldGod` (the Deity:
//!    `TAG_SCRIPT_DATA_NUM_2/3` are its Attack/Health), its trinkets, its hand (hidden cards are
//!    entities without a card ID), then, between `GameEntity` tag `1453` = 1 and = 0, its board:
//!    each minion followed by its enchantments. These boards are the warbands at the start of
//!    combat, before any Start of Combat.
//! 3. One more `1453` bracket copies the local player's own board (no hero package before it).
//! 4. Each warband entering the fight sets `BACON_CURRENT_COMBAT_PLAYER_ID` on its side's player
//!    entity (the local player for the friendly team, the dummy player for the enemy team),
//!    whose *player tags* are swapped to the entering player's (a `Bacon_TagTransferPlayerE`
//!    enchantment keeps the previous player's), its player enchantments are attached to that
//!    entity, and its board is created in `PLAY` (the local player's own minions move back from
//!    `SETASIDE`). Each entry is wrapped in a `1453` bracket; Start of Combat comes after it.
//! 5. `ATTACK` blocks (`PROPOSED_ATTACKER` / `PROPOSED_DEFENDER` on the game entity), `DEATHS`
//!    blocks and summons follow; the combat ends with the winning warband's hero attacking the
//!    losing team's hero for the damage (no hero attack: a draw).

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use seaglass::model::{BattleOutcome, Side};

/// A minion as the log shows it at a point of the combat (its board snapshot, or a hand card).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogMinion {
    pub entity_id: u32,
    pub card_id: String,
    pub name: String,
    pub attack: i32,
    pub health: i32,
    pub tier: u32,
    pub golden: bool,
    /// Its `CARDRACE` (`MECHANICAL`, `DEMON`, ...).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub race: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub taunt: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub divine_shield: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub windfury: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reborn: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub venomous: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stealth: bool,
    /// Card IDs of its enchantments (Magnetized cards, buffs, Dark Gifts, ...).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enchantments: Vec<String>,
    /// Its Dark Gifts (enchantments with `IS_NIGHTMARE_BONUS`), by name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dark_gifts: Vec<String>,
    /// `TAG_SCRIPT_DATA_NUM_1/2`: the numbers its card text shows (counters, stored stats).
    pub script_data: [i32; 2],
}

impl LogMinion {
    /// `"Name ★ 5/6 [taunt, divine_shield]"`.
    pub fn short(&self) -> String {
        let mut kws = Vec::new();
        for (on, kw) in [
            (self.taunt, "taunt"),
            (self.divine_shield, "divine_shield"),
            (self.windfury, "windfury"),
            (self.reborn, "reborn"),
            (self.venomous, "venomous"),
            (self.stealth, "stealth"),
        ] {
            if on {
                kws.push(kw);
            }
        }
        let gold = if self.golden { " ★" } else { "" };
        let kws = if kws.is_empty() {
            String::new()
        } else {
            format!(" [{}]", kws.join(", "))
        };
        format!("{}{gold} {}/{}{kws}", self.name, self.attack, self.health)
    }
}

/// A card in a player's hand at the start of combat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandCard {
    pub entity_id: u32,
    /// `None` when the log hides the card.
    pub card_id: Option<String>,
    pub name: String,
    pub card_type: String,
    /// The minion, for a visible minion card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minion: Option<LogMinion>,
}

/// An enchantment attached to a player entity (player effects and counters).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerEnchantment {
    pub card_id: String,
    pub name: String,
    pub script_data: [i32; 2],
}

/// Where a reconstructed board comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardSource {
    /// The local player's own board at the start of combat.
    LocalBoard,
    /// The player's combat package (sent at the start of combat; a warband that never fought).
    Package,
    /// The board that entered the fight, read before its Start of Combat.
    Entry,
    /// Not in the log.
    Missing,
}

/// One player's warband and context at the start of a combat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerSetup {
    pub player_id: i32,
    pub hero_card_id: String,
    pub hero_name: String,
    pub tier: u32,
    pub hero_health: i32,
    pub hero_armor: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hero_power: Option<String>,
    /// The hero power has a Start of Combat effect (`START_OF_COMBAT` tag).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hero_power_start_of_combat: bool,
    /// The hero power was used for this combat (`BACON_HERO_POWER_ACTIVATED` at the start of
    /// combat: Y'Shaarj's Embrace Your Rage).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hero_power_activated: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trinkets: Vec<String>,
    /// The number each trinket's text shows (`TAG_SCRIPT_DATA_NUM_1`), by card ID: `Hammer of
    /// Twilight`'s current bonus, which tells how many cards the player discarded.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub trinket_numbers: BTreeMap<String, i32>,
    /// Deity Attack/Health.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deity: Option<(i32, i32)>,
    pub board: Vec<LogMinion>,
    pub board_source: BoardSource,
    pub hand: Vec<HandCard>,
    /// Spells waiting for this combat (`OBJECTIVE`s in the `SECRET` zone, such as `Brood of
    /// Nozdormu`): the local player's at the start of combat, and each warband's when it enters
    /// (a warband that never fought: unknown).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_spells: Vec<HandCard>,
    /// Player tags read when the warband entered the fight (or at the start of combat for the
    /// local player): the counters the game transfers between teammates.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub player_tags: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub player_enchantments: Vec<PlayerEnchantment>,
    /// The player's tags are known (the local player, or a warband that entered the fight).
    pub tags_known: bool,
    /// The warband entered the fight.
    pub fought: bool,
    /// The package's board, when `board` is the board that entered the fight instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_board: Option<Vec<LogMinion>>,
}

/// One team in a combat: its players' warbands, the first fighter first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamSetup {
    /// `players[0]` fought first.
    pub players: Vec<PlayerSetup>,
}

/// A minion at a point of the server's combat log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitRef {
    pub entity_id: u32,
    pub card_id: String,
    pub name: String,
    pub golden: bool,
    pub attack: i32,
    pub health: i32,
}

impl UnitRef {
    pub fn short(&self) -> String {
        let gold = if self.golden { " ★" } else { "" };
        format!("{}{gold} {}/{}", self.name, self.attack, self.health)
    }
}

/// The source of a trigger: a minion, hero power, trinket, pending spell, Dark Gift or Deity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub entity_id: u32,
    pub card_id: String,
    pub name: String,
    /// Its `CARDTYPE`.
    pub kind: String,
}

/// One event of the server's combat log.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    /// A warband entered the fight (`BACON_CURRENT_COMBAT_PLAYER_ID`).
    Entry {
        side: Side,
        player_id: i32,
    },
    /// A trigger of a minion, hero power, trinket, Dark Gift or Deity during the fight.
    Trigger {
        side: Side,
        /// The source's entity.
        #[serde(default)]
        entity_id: u32,
        source: String,
        card_id: String,
        /// The source's `CARDTYPE` (`MINION`, `HERO_POWER`, `BATTLEGROUND_TRINKET`; `SPELL` /
        /// `BATTLEGROUND_SPELL`: a pending spell; `ENCHANTMENT`: a Dark Gift).
        kind: String,
        depth: usize,
        /// Minions whose stats or keywords changed during it: `(entity id, name)`.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        targets: Vec<(u32, String)>,
    },
    /// An attack, with both minions' stats when it was declared.
    Attack {
        side: Side,
        attacker: UnitRef,
        defender: UnitRef,
    },
    Summon {
        side: Side,
        unit: UnitRef,
        /// What summoned it: the innermost trigger's source.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<Source>,
    },
    Death {
        side: Side,
        unit: UnitRef,
    },
}

/// The real result of a combat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealResult {
    /// The friendly team's (side A's) perspective.
    pub outcome: BattleOutcome,
    /// The damage the losing team took (`0` on a draw).
    pub damage: i32,
    /// The player whose hero dealt the damage (the winning warband's player).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub winner_player_id: Option<i32>,
    /// The winning side's minions when its hero attacked.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub survivors: Vec<LogMinion>,
}

/// A combat reconstructed as engine input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatSetup {
    /// The round (`TurnReplay::turn`).
    pub turn: u32,
    pub is_duos: bool,
    /// The friendly team (side A).
    pub team_a: TeamSetup,
    /// The enemy team (side B).
    pub team_b: TeamSetup,
    /// The combat damage cap in effect (`BACON_COMBAT_DAMAGE_CAP`), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage_cap: Option<i32>,
    /// The game's Deity (`BACON_GLOBAL_OLD_GOD_DBID`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub deity_dbid: String,
    pub result: RealResult,
    pub events: Vec<ServerEvent>,
    /// Both boards at every warband entry and before every attack (including the winning
    /// hero's, so the last frame shows the survivors), for viewers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<CombatFrame>,
    /// Reconstruction caveats.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// Both boards at one moment of a combat, as the log shows them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombatFrame {
    /// How many of the combat's [`CombatSetup::events`] had been recorded: the boards are as
    /// they stand after `events[..events]`.
    pub events: usize,
    /// The friendly side's minions in play.
    pub board_a: Vec<LogMinion>,
    /// The enemy side's minions in play.
    pub board_b: Vec<LogMinion>,
}

impl CombatSetup {
    /// The real attacks, in order.
    pub fn attacks(&self) -> impl Iterator<Item = (&Side, &UnitRef, &UnitRef)> {
        self.events.iter().filter_map(|e| match e {
            ServerEvent::Attack {
                side,
                attacker,
                defender,
            } => Some((side, attacker, defender)),
            _ => None,
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Entity tracking
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
struct Ent {
    card_id: String,
    name: String,
    tags: HashMap<String, String>,
}

impl Ent {
    fn int(&self, key: &str) -> i32 {
        self.tags.get(key).and_then(|v| v.parse().ok()).unwrap_or(0)
    }
    fn str(&self, key: &str) -> &str {
        self.tags.get(key).map(String::as_str).unwrap_or("")
    }
    fn health(&self) -> i32 {
        self.int("HEALTH") - self.int("DAMAGE")
    }
}

/// Parses `[entityName=X id=N zone=Z zonePos=P cardId=C player=Q]` into `(id, name, card_id)`.
fn bracket(s: &str) -> Option<(u32, Option<String>, Option<String>)> {
    crate::parse_bracket_entity(s)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BracketOwner {
    /// Not decided yet (its first minion or entry decides).
    Undecided,
    /// A package's board.
    Package(usize),
    /// The copy of the local player's board.
    LocalCopy,
    /// One or more warbands entering the fight.
    Entry,
}

struct Package {
    hero: u32,
    player_id: i32,
    hero_power: Option<u32>,
    deity: Option<u32>,
    trinkets: Vec<u32>,
    /// Entities created between the hero and its board (hand cards, player enchantments).
    pre_board: Vec<u32>,
    board: Vec<u32>,
    board_started: bool,
    /// The player as it was when its board bracket closed (later the server reuses or resets
    /// these entities).
    snapshot: Option<PlayerSetup>,
}

struct Entry {
    side: Side,
    player_id: i32,
    /// Snapshot taken when its bracket closed.
    snapshot: Option<EntrySnapshot>,
}

struct EntrySnapshot {
    tags: BTreeMap<String, String>,
    enchantments: Vec<PlayerEnchantment>,
    board: Vec<LogMinion>,
    /// Its side's `SECRET` zone: the entering player's pending spells.
    pending: Vec<HandCard>,
    hero: Option<u32>,
}

struct AttackTrack {
    indent: usize,
    attacker: u32,
    defender: u32,
    recorded: bool,
    damage_meta: Option<i32>,
}

struct CombatBuilder {
    turn: u32,
    damage_cap: Option<i32>,
    deity_dbid: String,
    fights_started: bool,
    packages: Vec<Package>,
    bracket: Option<BracketOwner>,
    local_copy: Vec<u32>,
    local_board: Vec<LogMinion>,
    local_hand: Vec<HandCard>,
    local_pending: Vec<HandCard>,
    local_hero: u32,
    /// The local hero's tags at the start of combat.
    local_hero_tags: Ent,
    /// The local hero power's tags at the start of combat.
    local_hero_power: Option<Ent>,
    local_tags: BTreeMap<String, String>,
    local_enchantments: Vec<PlayerEnchantment>,
    entries: Vec<Entry>,
    events: Vec<ServerEvent>,
    frames: Vec<CombatFrame>,
    attack: Option<AttackTrack>,
    result: Option<RealResult>,
    /// Minions already reported as summoned.
    summoned: std::collections::HashSet<u32>,
}

struct Tracker<'a> {
    ents: HashMap<u32, Ent>,
    card_names: &'a HashMap<String, String>,
    game_eid: u32,
    local_pid: i32,
    local_eid: u32,
    dummy_pid: i32,
    dummy_eid: u32,
    local_name: String,
    round: u32,
    combat: Option<CombatBuilder>,
    done: Vec<CombatSetup>,
    /// Blocks open: `(indent, block type, entity id)`.
    blocks: Vec<(usize, String, u32)>,
    /// For each open block, the index of the `Trigger` event it recorded (if any).
    block_triggers: Vec<Option<usize>>,
}

/// Tags whose change shows a trigger affecting a minion (stats and keywords).
const AFFECTING_TAGS: [&str; 11] = [
    "ATK",
    "HEALTH",
    "DAMAGE",
    "REBORN",
    "DIVINE_SHIELD",
    "TAUNT",
    "WINDFURY",
    "MEGA_WINDFURY",
    "POISONOUS",
    "VENOMOUS",
    "STEALTH",
];

impl<'a> Tracker<'a> {
    fn new(card_names: &'a HashMap<String, String>) -> Self {
        Self {
            ents: HashMap::new(),
            card_names,
            game_eid: 1,
            local_pid: 0,
            local_eid: 0,
            dummy_pid: 0,
            dummy_eid: 0,
            local_name: String::new(),
            round: 0,
            combat: None,
            done: Vec::new(),
            blocks: Vec::new(),
            block_triggers: Vec::new(),
        }
    }

    fn resolve(&mut self, s: &str) -> Option<u32> {
        let s = s.trim();
        if s.is_empty() || s == "0" {
            return None;
        }
        if let Some((id, name, cid)) = bracket(s) {
            let e = self.ents.entry(id).or_default();
            if let Some(n) = name {
                if !n.starts_with("UNKNOWN ENTITY") {
                    e.name = n;
                }
            }
            if let Some(c) = cid {
                if !c.is_empty() {
                    e.card_id = c;
                }
            }
            return Some(id);
        }
        if let Ok(id) = s.parse::<u32>() {
            return Some(id);
        }
        if s == "GameEntity" {
            return Some(self.game_eid);
        }
        if (!self.local_name.is_empty() && s == self.local_name) || s.contains('#') {
            return (self.local_eid != 0).then_some(self.local_eid);
        }
        (self.dummy_eid != 0).then_some(self.dummy_eid)
    }

    fn ent(&self, id: u32) -> Option<&Ent> {
        self.ents.get(&id)
    }

    fn name_of(&self, id: u32) -> String {
        let Some(e) = self.ents.get(&id) else {
            return format!("#{id}");
        };
        if !e.name.is_empty() {
            return e.name.clone();
        }
        self.card_names
            .get(&e.card_id)
            .cloned()
            .unwrap_or_else(|| e.card_id.clone())
    }

    fn side_of_controller(&self, controller: i32) -> Side {
        if controller == self.local_pid {
            Side::A
        } else {
            Side::B
        }
    }

    fn minion(&self, id: u32) -> LogMinion {
        let e = self.ents.get(&id).cloned().unwrap_or_default();
        let attached: BTreeMap<u32, &Ent> = self
            .ents
            .iter()
            .filter(|(_, x)| {
                x.int("ATTACHED") == id as i32
                    && x.str("CARDTYPE") == "ENCHANTMENT"
                    && x.str("ZONE") != "REMOVEDFROMGAME"
                    && x.str("ZONE") != "GRAVEYARD"
            })
            .map(|(eid, x)| (*eid, x))
            .collect();
        let enchantments = attached.values().map(|x| x.card_id.clone()).collect();
        let mut dark_gifts: Vec<String> = attached
            .values()
            .filter(|x| x.int("IS_NIGHTMARE_BONUS") > 0)
            .map(|x| self.dark_gift_name(&x.card_id))
            .collect();
        dark_gifts.dedup();
        LogMinion {
            entity_id: id,
            card_id: e.card_id.clone(),
            name: self.name_of(id),
            attack: e.int("ATK"),
            health: e.health(),
            tier: e.int("TECH_LEVEL").max(1) as u32,
            golden: e.int("PREMIUM") > 0,
            race: e.str("CARDRACE").to_string(),
            taunt: e.int("TAUNT") > 0,
            divine_shield: e.int("DIVINE_SHIELD") > 0,
            windfury: e.int("WINDFURY") > 0,
            reborn: e.int("REBORN") > 0,
            venomous: e.int("VENOMOUS") > 0 || e.int("POISONOUS") > 0,
            stealth: e.int("STEALTH") > 0,
            enchantments,
            dark_gifts,
            script_data: [
                e.int("TAG_SCRIPT_DATA_NUM_1"),
                e.int("TAG_SCRIPT_DATA_NUM_2"),
            ],
        }
    }

    /// The name of the Dark Gift an enchantment comes from (`BG36_MidGameEffect_000t9e2` →
    /// `BG36_MidGameEffect_000t9`, "Admiration").
    fn dark_gift_name(&self, enchantment: &str) -> String {
        let base = enchantment
            .find("_000t")
            .and_then(|i| {
                let digits = enchantment[i + 5..]
                    .bytes()
                    .take_while(u8::is_ascii_digit)
                    .count();
                (digits > 0).then(|| &enchantment[..i + 5 + digits])
            })
            .unwrap_or(enchantment);
        self.card_names
            .get(base)
            .or_else(|| self.card_names.get(enchantment))
            .cloned()
            .unwrap_or_else(|| enchantment.to_string())
    }

    fn unit_ref(&self, id: u32) -> UnitRef {
        let e = self.ents.get(&id).cloned().unwrap_or_default();
        UnitRef {
            entity_id: id,
            card_id: e.card_id.clone(),
            name: self.name_of(id),
            golden: e.int("PREMIUM") > 0,
            attack: e.int("ATK"),
            health: e.health(),
        }
    }

    fn hand_card(&self, id: u32) -> HandCard {
        let e = self.ents.get(&id).cloned().unwrap_or_default();
        let ctype = e.str("CARDTYPE").to_string();
        HandCard {
            entity_id: id,
            card_id: (!e.card_id.is_empty()).then(|| e.card_id.clone()),
            name: if e.card_id.is_empty() {
                "(hidden)".to_string()
            } else {
                self.name_of(id)
            },
            minion: (ctype == "MINION").then(|| self.minion(id)),
            card_type: ctype,
        }
    }

    /// The minions in `zone` controlled by `controller`, in zone order.
    fn zone_minions(&self, controller: i32, zone: &str) -> Vec<u32> {
        let mut v: Vec<(i32, u32)> = self
            .ents
            .iter()
            .filter(|(_, e)| {
                e.str("ZONE") == zone
                    && e.str("CARDTYPE") == "MINION"
                    && e.int("CONTROLLER") == controller
            })
            .map(|(id, e)| (e.int("ZONE_POSITION"), *id))
            .collect();
        v.sort();
        v.into_iter().map(|(_, id)| id).collect()
    }

    /// Spells waiting in `controller`'s `SECRET` zone (`OBJECTIVE`s such as `Brood of Nozdormu`:
    /// "at the start of your next combat ...").
    fn pending_spell_ids(&self, controller: i32) -> Vec<u32> {
        let mut v: Vec<u32> = self
            .ents
            .iter()
            .filter(|(_, e)| {
                e.str("ZONE") == "SECRET"
                    && e.int("CONTROLLER") == controller
                    && matches!(e.str("CARDTYPE"), "SPELL" | "BATTLEGROUND_SPELL")
                    && !e.card_id.is_empty()
                    && e.card_id != "BG_OldGod"
            })
            .map(|(id, _)| *id)
            .collect();
        v.sort();
        v
    }

    fn hand_ids(&self, controller: i32) -> Vec<u32> {
        let mut v: Vec<(i32, u32)> = self
            .ents
            .iter()
            .filter(|(_, e)| {
                e.str("ZONE") == "HAND"
                    && e.int("CONTROLLER") == controller
                    && matches!(
                        e.str("CARDTYPE"),
                        "MINION" | "SPELL" | "BATTLEGROUND_SPELL" | ""
                    )
            })
            .map(|(id, e)| (e.int("ZONE_POSITION"), *id))
            .collect();
        v.sort();
        v.into_iter().map(|(_, id)| id).collect()
    }

    fn player_enchantments(&self, player_eid: u32) -> Vec<PlayerEnchantment> {
        let mut v: Vec<(u32, PlayerEnchantment)> = self
            .ents
            .iter()
            .filter(|(_, e)| {
                e.int("ATTACHED") == player_eid as i32
                    && e.str("CARDTYPE") == "ENCHANTMENT"
                    && e.str("ZONE") == "PLAY"
            })
            .map(|(id, e)| {
                (
                    *id,
                    PlayerEnchantment {
                        card_id: e.card_id.clone(),
                        name: self.name_of(*id),
                        script_data: [
                            e.int("TAG_SCRIPT_DATA_NUM_1"),
                            e.int("TAG_SCRIPT_DATA_NUM_2"),
                        ],
                    },
                )
            })
            .collect();
        v.sort_by_key(|(id, _)| *id);
        v.into_iter().map(|(_, e)| e).collect()
    }

    fn player_tags(&self, player_eid: u32) -> BTreeMap<String, String> {
        self.ents
            .get(&player_eid)
            .map(|e| e.tags.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    }

    // -----------------------------------------------------------------------------------------
    // Events
    // -----------------------------------------------------------------------------------------

    fn in_combat(&self) -> bool {
        self.ent(self.game_eid)
            .is_some_and(|g| g.int("BACON_IN_COMBAT_PHASE") == 1)
    }

    fn start_combat(&mut self) {
        self.block_triggers.iter_mut().for_each(|t| *t = None);
        let local_hero = self
            .ent(self.local_eid)
            .map_or(0, |p| p.int("HERO_ENTITY") as u32);
        let damage_cap = self.ent(self.game_eid).and_then(|g| {
            (g.int("BACON_COMBAT_DAMAGE_CAP_ENABLED") == 1)
                .then(|| g.int("BACON_COMBAT_DAMAGE_CAP"))
                .filter(|&c| c > 0)
        });
        let local_board = self
            .zone_minions(self.local_pid, "PLAY")
            .into_iter()
            .map(|id| self.minion(id))
            .collect();
        let local_hand = self
            .hand_ids(self.local_pid)
            .into_iter()
            .map(|id| self.hand_card(id))
            .collect();
        let local_pending = self
            .pending_spell_ids(self.local_pid)
            .into_iter()
            .map(|id| self.hand_card(id))
            .collect();
        let deity_dbid = self
            .ent(self.game_eid)
            .map(|g| g.str("BACON_GLOBAL_OLD_GOD_DBID").to_string())
            .unwrap_or_default();
        self.combat = Some(CombatBuilder {
            turn: self.round,
            damage_cap,
            deity_dbid,
            fights_started: false,
            packages: Vec::new(),
            bracket: None,
            local_copy: Vec::new(),
            local_board,
            local_hand,
            local_pending,
            local_hero,
            local_hero_tags: self.ent(local_hero).cloned().unwrap_or_default(),
            local_hero_power: self.local_hero_power(),
            local_tags: self.player_tags(self.local_eid),
            local_enchantments: self.player_enchantments(self.local_eid),
            entries: Vec::new(),
            events: Vec::new(),
            frames: Vec::new(),
            attack: None,
            result: None,
            summoned: Default::default(),
        });
    }

    /// The local player's hero power: the `HERO_POWER` entity they control in `PLAY` (the
    /// lowest id, as the reconstruction has always taken the first one).
    fn local_hero_power(&self) -> Option<Ent> {
        let mut powers: Vec<(&u32, &Ent)> = self
            .ents
            .iter()
            .filter(|(_, e)| {
                e.int("CONTROLLER") == self.local_pid
                    && e.str("ZONE") == "PLAY"
                    && e.str("CARDTYPE") == "HERO_POWER"
            })
            .collect();
        powers.sort_by_key(|(id, _)| **id);
        powers.first().map(|(_, e)| (*e).clone())
    }

    fn on_created(&mut self, id: u32) {
        self.note_enchantment(id);
        let Some(e) = self.ents.get(&id) else {
            return;
        };
        let ctype = e.str("CARDTYPE").to_string();
        let zone = e.str("ZONE").to_string();
        let controller = e.int("CONTROLLER");
        let card_id = e.card_id.clone();
        let start_of_combat = e.int("START_OF_COMBAT") > 0;
        let local_pid = self.local_pid;
        let in_combat = self.in_combat();
        let Some(c) = self.combat.as_mut() else {
            return;
        };
        if !in_combat {
            return;
        }
        if !c.fights_started && zone == "SETASIDE" {
            if ctype == "HERO" {
                let player_id = self.ents[&id].int("PLAYER_ID");
                c.packages.push(Package {
                    hero: id,
                    player_id,
                    hero_power: None,
                    deity: None,
                    trinkets: Vec::new(),
                    pre_board: Vec::new(),
                    board: Vec::new(),
                    board_started: false,
                    snapshot: None,
                });
                return;
            }
            match c.bracket {
                Some(BracketOwner::Undecided) if ctype == "MINION" => {
                    let waiting = c
                        .packages
                        .iter()
                        .rposition(|p| !p.board_started)
                        .filter(|&i| i + 1 == c.packages.len());
                    match waiting {
                        Some(i) => {
                            c.packages[i].board_started = true;
                            c.packages[i].board.push(id);
                            c.bracket = Some(BracketOwner::Package(i));
                        }
                        None => {
                            c.local_copy.push(id);
                            c.bracket = Some(BracketOwner::LocalCopy);
                        }
                    }
                    return;
                }
                Some(BracketOwner::Package(i)) if ctype == "MINION" => {
                    c.packages[i].board.push(id);
                    return;
                }
                Some(BracketOwner::LocalCopy) if ctype == "MINION" => {
                    c.local_copy.push(id);
                    return;
                }
                Some(_) => return,
                None => {}
            }
            let Some(p) = c.packages.last_mut().filter(|p| !p.board_started) else {
                return;
            };
            match ctype.as_str() {
                "HERO_POWER" => {
                    p.hero_power = Some(id);
                    let _ = start_of_combat;
                }
                "BATTLEGROUND_TRINKET" => p.trinkets.push(id),
                "SPELL" if card_id == "BG_OldGod" => p.deity = Some(id),
                "ENCHANTMENT" => {}
                _ => p.pre_board.push(id),
            }
            return;
        }
        if c.fights_started && ctype == "MINION" && zone == "PLAY" {
            if c.bracket == Some(BracketOwner::Entry) {
                return;
            }
            let side = if controller == local_pid {
                Side::A
            } else {
                Side::B
            };
            self.record_summon(id, side);
        }
    }

    fn record_summon(&mut self, id: u32, side: Side) {
        let unit = self.unit_ref(id);
        // What summoned it: the innermost trigger or power block's source.
        let by = self
            .blocks
            .iter()
            .rev()
            .find(|(_, btype, ent)| (btype == "TRIGGER" || btype == "POWER") && *ent != 0)
            .map(|&(_, _, ent)| Source {
                entity_id: ent,
                card_id: self.ent(ent).map(|e| e.card_id.clone()).unwrap_or_default(),
                name: self.name_of(ent),
                kind: self.ent(ent).map_or("", |e| e.str("CARDTYPE")).to_string(),
            });
        if let Some(c) = self.combat.as_mut() {
            if c.summoned.insert(id) {
                c.events.push(ServerEvent::Summon { side, unit, by });
            }
        }
    }

    /// An entity revealed by `SHOW_ENTITY` (the Deity is created hidden, then shown in `PLAY`).
    /// An enchantment attached during a trigger: the trigger affects that minion (its stats change
    /// only after the block).
    fn note_enchantment(&mut self, id: u32) {
        let attached = self
            .ents
            .get(&id)
            .filter(|e| e.str("CARDTYPE") == "ENCHANTMENT")
            .map_or(0, |e| e.int("ATTACHED") as u32);
        if attached != 0 && self.combat.is_some() {
            self.note_target(attached);
        }
    }

    fn on_shown(&mut self, id: u32) {
        self.note_enchantment(id);
        let Some(e) = self.ents.get(&id) else {
            return;
        };
        if e.str("CARDTYPE") != "MINION" || e.str("ZONE") != "PLAY" || !self.in_combat() {
            return;
        }
        let side = self.side_of_controller(e.int("CONTROLLER"));
        let fights = self
            .combat
            .as_ref()
            .is_some_and(|c| c.fights_started && c.bracket != Some(BracketOwner::Entry));
        if fights {
            self.record_summon(id, side);
        }
    }

    fn on_bracket(&mut self, open: bool) {
        let Some(c) = self.combat.as_mut() else {
            return;
        };
        if open {
            c.bracket = Some(if c.fights_started {
                BracketOwner::Entry
            } else {
                BracketOwner::Undecided
            });
            // A package's hand: what it created before its board (hidden cards, visible ones).
            return;
        }
        let owner = c.bracket.take();
        let mut closed = None;
        if let Some(BracketOwner::Package(i)) = owner {
            c.packages[i].board_started = true;
            closed = Some(i);
        }
        if owner == Some(BracketOwner::Undecided) {
            // An empty bracket: the waiting package's board is empty.
            if let Some(i) = c.packages.iter().rposition(|p| !p.board_started) {
                if i + 1 == c.packages.len() {
                    c.packages[i].board_started = true;
                    closed = Some(i);
                }
            }
        }
        if let Some(i) = closed {
            let snap = self.package_player(&self.combat.as_ref().unwrap().packages[i]);
            self.combat.as_mut().unwrap().packages[i].snapshot = Some(snap);
        }
        if owner == Some(BracketOwner::Entry) {
            self.snapshot_entries(true);
        }
    }

    /// Snapshots the entries not snapshotted yet. Unless `force`, only those whose board is in
    /// play already (a Start of Combat trigger can come before the other side's board exists).
    fn snapshot_entries(&mut self, force: bool) {
        let pending: Vec<(usize, Side, i32)> = match self.combat.as_ref() {
            Some(c) => c
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.snapshot.is_none())
                .map(|(i, e)| (i, e.side, e.player_id))
                .collect(),
            None => return,
        };
        for (i, side, pid) in pending {
            let (player_eid, controller) = match side {
                Side::A => (self.local_eid, self.local_pid),
                Side::B => (self.dummy_eid, self.dummy_pid),
            };
            if !force {
                let c = self.combat.as_ref().unwrap();
                let expected = if pid == self.local_pid {
                    c.local_board.len()
                } else {
                    c.packages
                        .iter()
                        .find(|p| p.player_id == pid)
                        .map_or(1, |p| p.board.len())
                };
                if self.zone_minions(controller, "PLAY").len() < expected {
                    continue;
                }
            }
            let snapshot = EntrySnapshot {
                tags: self.player_tags(player_eid),
                enchantments: self.player_enchantments(player_eid),
                board: self
                    .zone_minions(controller, "PLAY")
                    .into_iter()
                    .map(|id| self.minion(id))
                    .collect(),
                pending: self
                    .pending_spell_ids(controller)
                    .into_iter()
                    .map(|id| self.hand_card(id))
                    .collect(),
                hero: self
                    .ent(player_eid)
                    .map(|p| p.int("HERO_ENTITY") as u32)
                    .filter(|&h| h != 0),
            };
            if let Some(c) = self.combat.as_mut() {
                c.entries[i].snapshot = Some(snapshot);
            }
            self.snapshot_frame();
        }
    }

    /// Record both boards as they stand ([`CombatFrame`]), unless nothing changed since the last
    /// frame.
    fn snapshot_frame(&mut self) {
        if !self.combat.as_ref().is_some_and(|c| c.fights_started) {
            return;
        }
        let board = |t: &Self, controller: i32| -> Vec<LogMinion> {
            t.zone_minions(controller, "PLAY")
                .into_iter()
                .map(|id| t.frame_minion(id))
                .collect()
        };
        let board_a = board(self, self.local_pid);
        let board_b = board(self, self.dummy_pid);
        let c = self.combat.as_mut().unwrap();
        let events = c.events.len();
        if c.frames
            .last()
            .is_some_and(|f| f.board_a == board_a && f.board_b == board_b)
        {
            return;
        }
        c.frames.push(CombatFrame {
            events,
            board_a,
            board_b,
        });
    }

    /// [`Self::minion`] without its enchantments (cheap enough for every attack).
    fn frame_minion(&self, id: u32) -> LogMinion {
        let e = self.ents.get(&id).cloned().unwrap_or_default();
        LogMinion {
            entity_id: id,
            card_id: e.card_id.clone(),
            name: self.name_of(id),
            attack: e.int("ATK"),
            health: e.health(),
            tier: e.int("TECH_LEVEL").max(1) as u32,
            golden: e.int("PREMIUM") > 0,
            race: e.str("CARDRACE").to_string(),
            taunt: e.int("TAUNT") > 0,
            divine_shield: e.int("DIVINE_SHIELD") > 0,
            windfury: e.int("WINDFURY") > 0,
            reborn: e.int("REBORN") > 0,
            venomous: e.int("VENOMOUS") > 0 || e.int("POISONOUS") > 0,
            stealth: e.int("STEALTH") > 0,
            enchantments: Vec::new(),
            dark_gifts: Vec::new(),
            script_data: [
                e.int("TAG_SCRIPT_DATA_NUM_1"),
                e.int("TAG_SCRIPT_DATA_NUM_2"),
            ],
        }
    }

    fn on_tag(&mut self, id: u32, tag: &str, old: Option<String>, value: &str) {
        if AFFECTING_TAGS.contains(&tag) && self.combat.is_some() {
            self.note_target(id);
        }
        if id == self.game_eid {
            match tag {
                "TURN" => {
                    let t: u32 = value.parse().unwrap_or(0);
                    self.round = t.div_ceil(2);
                }
                "BACON_IN_COMBAT_PHASE" => {
                    if value == "1" && old.as_deref() != Some("1") {
                        self.finish_combat();
                        self.start_combat();
                    } else if value == "0" {
                        self.finish_combat();
                    }
                }
                "1453" if self.combat.is_some() => self.on_bracket(value == "1"),
                "PROPOSED_ATTACKER" | "PROPOSED_DEFENDER" => self.on_proposed(tag, value),
                _ => {}
            }
            return;
        }
        if self.combat.is_none() || !self.in_combat() {
            return;
        }
        let side = if id == self.local_eid {
            Some(Side::A)
        } else if id == self.dummy_eid {
            Some(Side::B)
        } else {
            None
        };
        if let (Some(side), "BACON_CURRENT_COMBAT_PLAYER_ID") = (side, tag) {
            let pid: i32 = value.parse().unwrap_or(0);
            if pid != 0 {
                let in_bracket = {
                    let c = self.combat.as_mut().unwrap();
                    c.fights_started = true;
                    if c.bracket == Some(BracketOwner::Undecided) {
                        c.bracket = Some(BracketOwner::Entry);
                    }
                    c.entries.push(Entry {
                        side,
                        player_id: pid,
                        snapshot: None,
                    });
                    c.events.push(ServerEvent::Entry {
                        side,
                        player_id: pid,
                    });
                    c.bracket == Some(BracketOwner::Entry)
                };
                if !in_bracket {
                    self.snapshot_entries(true);
                }
            }
            return;
        }
        if tag == "ZONE" {
            let Some(e) = self.ents.get(&id) else {
                return;
            };
            if e.str("CARDTYPE") != "MINION" {
                return;
            }
            let fights = self.combat.as_ref().is_some_and(|c| c.fights_started);
            if !fights {
                return;
            }
            let side = self.side_of_controller(e.int("CONTROLLER"));
            let old = old.unwrap_or_default();
            if value == "GRAVEYARD" && old == "PLAY" {
                let unit = self.unit_ref(id);
                self.combat
                    .as_mut()
                    .unwrap()
                    .events
                    .push(ServerEvent::Death { side, unit });
            } else if value == "PLAY" && old != "PLAY" && !old.is_empty() {
                let c = self.combat.as_ref().unwrap();
                if c.bracket != Some(BracketOwner::Entry) {
                    self.record_summon(id, side);
                }
            }
        }
    }

    fn on_proposed(&mut self, tag: &str, value: &str) {
        let id: u32 = value.parse().unwrap_or(0);
        if id == 0 {
            return;
        }
        let ready = {
            let Some(c) = self.combat.as_mut() else {
                return;
            };
            let Some(a) = c.attack.as_mut() else {
                return;
            };
            if tag == "PROPOSED_ATTACKER" {
                a.attacker = id;
            } else {
                a.defender = id;
            }
            (a.attacker != 0 && a.defender != 0 && !a.recorded).then_some((a.attacker, a.defender))
        };
        let Some((att, def)) = ready else {
            return;
        };
        let is_hero = |t: &Self, x: u32| t.ent(x).is_some_and(|e| e.str("CARDTYPE") == "HERO");
        if is_hero(self, att) || is_hero(self, def) {
            return;
        }
        let side = self.side_of_controller(self.ent(att).map_or(0, |e| e.int("CONTROLLER")));
        let attacker = self.unit_ref(att);
        let defender = self.unit_ref(def);
        let c = self.combat.as_mut().unwrap();
        c.attack.as_mut().unwrap().recorded = true;
        c.events.push(ServerEvent::Attack {
            side,
            attacker,
            defender,
        });
    }

    fn on_block_start(&mut self, indent: usize, s: &str) {
        let btype = s
            .strip_prefix("BLOCK_START BlockType=")
            .and_then(|r| r.split_whitespace().next())
            .unwrap_or("")
            .to_string();
        let ent = s
            .find(" Entity=")
            .and_then(|p| {
                let sub = &s[p + 8..];
                let end = sub.find(" EffectCardId=")?;
                Some(sub[..end].to_string())
            })
            .and_then(|r| self.resolve(&r))
            .unwrap_or(0);
        self.blocks.push((indent, btype.clone(), ent));
        self.block_triggers.push(None);
        let fights = self
            .combat
            .as_ref()
            .is_some_and(|c| c.fights_started && c.result.is_none());
        if !fights || !self.in_combat() {
            return;
        }
        if btype == "ATTACK" {
            self.snapshot_entries(true);
            self.snapshot_frame();
            let c = self.combat.as_mut().unwrap();
            c.attack = Some(AttackTrack {
                indent,
                attacker: 0,
                defender: 0,
                recorded: false,
                damage_meta: None,
            });
        } else if btype == "TRIGGER" || btype == "POWER" {
            let Some(e) = self.ent(ent) else {
                return;
            };
            let ctype = e.str("CARDTYPE");
            let interesting = matches!(ctype, "MINION" | "HERO_POWER" | "BATTLEGROUND_TRINKET")
                || e.card_id == "BG_OldGod"
                || (ctype == "ENCHANTMENT" && e.int("IS_NIGHTMARE_BONUS") > 0)
                || (e.str("ZONE") == "SECRET" && matches!(ctype, "SPELL" | "BATTLEGROUND_SPELL"));
            if !interesting {
                return;
            }
            // Start of Combat can trigger inside the entry bracket: the boards that entered are
            // read before it.
            self.snapshot_entries(false);
            let Some(e) = self.ent(ent) else {
                return;
            };
            let side = self.side_of_controller(e.int("CONTROLLER"));
            let card_id = e.card_id.clone();
            let kind = e.str("CARDTYPE").to_string();
            let source = self.name_of(ent);
            let depth = self.blocks.len() - 1;
            let events = &mut self.combat.as_mut().unwrap().events;
            events.push(ServerEvent::Trigger {
                side,
                entity_id: ent,
                source,
                card_id,
                kind,
                depth,
                targets: Vec::new(),
            });
            let idx = events.len() - 1;
            if let Some(slot) = self.block_triggers.last_mut() {
                *slot = Some(idx);
            }
        }
    }

    /// Notes `id` as affected by the innermost recorded trigger (a stat or keyword change).
    fn note_target(&mut self, id: u32) {
        let Some(idx) = self.block_triggers.iter().rev().flatten().next().copied() else {
            return;
        };
        if !self.ent(id).is_some_and(|e| e.str("CARDTYPE") == "MINION") {
            return;
        }
        let name = self.name_of(id);
        if let Some(c) = self.combat.as_mut() {
            if let Some(ServerEvent::Trigger { targets, .. }) = c.events.get_mut(idx) {
                if !targets.iter().any(|(t, _)| *t == id) {
                    targets.push((id, name));
                }
            }
        }
    }

    fn on_block_end(&mut self) {
        self.block_triggers.pop();
        let Some((indent, btype, _)) = self.blocks.pop() else {
            return;
        };
        // The boards once a round of deaths has resolved (the last one shows how a combat
        // without a hero attack, a draw, ends; later the server restores the boards).
        if btype == "DEATHS"
            && self
                .combat
                .as_ref()
                .is_some_and(|c| c.fights_started && c.result.is_none())
            && self.in_combat()
        {
            self.snapshot_frame();
        }
        if btype != "ATTACK" {
            return;
        }
        let Some(c) = self.combat.as_mut() else {
            return;
        };
        let Some(a) = c.attack.as_ref() else {
            return;
        };
        if a.indent != indent {
            return;
        }
        let a = c.attack.take().unwrap();
        let (att, def) = (a.attacker, a.defender);
        if att == 0 || def == 0 {
            return;
        }
        let Some(att_e) = self.ents.get(&att) else {
            return;
        };
        if att_e.str("CARDTYPE") != "HERO" {
            return;
        }
        // The winning warband's hero hits the losing team's hero.
        let damage = a.damage_meta.unwrap_or(att_e.int("ATK"));
        let controller = att_e.int("CONTROLLER");
        let side = self.side_of_controller(controller);
        let winner = att_e.int("PLAYER_ID");
        let survivors = self
            .zone_minions(controller, "PLAY")
            .into_iter()
            .map(|id| self.minion(id))
            .collect();
        let c = self.combat.as_mut().unwrap();
        if c.result.is_none() {
            c.result = Some(RealResult {
                outcome: match side {
                    Side::A => BattleOutcome::AWin,
                    Side::B => BattleOutcome::BWin,
                },
                damage,
                winner_player_id: (winner != 0).then_some(winner),
                survivors,
            });
        }
    }

    fn on_meta(&mut self, s: &str) {
        if !s.contains("Meta=DAMAGE") {
            return;
        }
        let Some(c) = self.combat.as_mut() else {
            return;
        };
        let Some(a) = c.attack.as_mut() else {
            return;
        };
        if a.damage_meta.is_none() {
            a.damage_meta = s
                .split("Data=")
                .nth(1)
                .and_then(|r| r.split_whitespace().next())
                .and_then(|v| v.parse().ok());
        }
    }

    // -----------------------------------------------------------------------------------------
    // Finishing a combat
    // -----------------------------------------------------------------------------------------

    fn finish_combat(&mut self) {
        let Some(c) = self.combat.take() else {
            return;
        };
        if c.entries.is_empty() {
            return;
        }
        let setup = self.build_setup(c);
        self.done.push(setup);
    }

    fn package_player(&self, p: &Package) -> PlayerSetup {
        let hero = self.ent(p.hero).cloned().unwrap_or_default();
        let hero_power = p.hero_power.and_then(|id| self.ent(id));
        let deity = p.deity.and_then(|id| self.ent(id)).map(|d| {
            (
                d.int("TAG_SCRIPT_DATA_NUM_2"),
                d.int("TAG_SCRIPT_DATA_NUM_3"),
            )
        });
        let hand = p
            .pre_board
            .iter()
            .filter(|&&id| {
                self.ent(id).is_some_and(|e| {
                    !matches!(
                        e.str("CARDTYPE"),
                        "ENCHANTMENT" | "HERO_POWER" | "BATTLEGROUND_TRINKET" | "HERO"
                    )
                })
            })
            .map(|&id| self.hand_card(id))
            .collect();
        PlayerSetup {
            player_id: p.player_id,
            hero_card_id: hero.card_id.clone(),
            hero_name: self.name_of(p.hero),
            tier: hero.int("PLAYER_TECH_LEVEL").max(1) as u32,
            hero_health: hero.health(),
            hero_armor: hero.int("ARMOR"),
            hero_power: hero_power.map(|h| h.card_id.clone()),
            hero_power_start_of_combat: hero_power.is_some_and(|h| h.int("START_OF_COMBAT") > 0),
            hero_power_activated: hero_power
                .is_some_and(|h| h.int("BACON_HERO_POWER_ACTIVATED") > 0),
            trinkets: p
                .trinkets
                .iter()
                .filter_map(|&id| self.ent(id))
                .filter(|t| t.int("BACON_IS_POTENTIAL_TRINKET") == 0)
                .map(|t| t.card_id.clone())
                .collect(),
            trinket_numbers: p
                .trinkets
                .iter()
                .filter_map(|&id| self.ent(id))
                .filter(|t| t.int("BACON_IS_POTENTIAL_TRINKET") == 0)
                .map(|t| (t.card_id.clone(), t.int("TAG_SCRIPT_DATA_NUM_1")))
                .collect(),
            deity,
            board: p.board.iter().map(|&id| self.minion(id)).collect(),
            board_source: BoardSource::Package,
            hand,
            pending_spells: Vec::new(),
            player_tags: BTreeMap::new(),
            player_enchantments: Vec::new(),
            tags_known: false,
            fought: false,
            package_board: None,
        }
    }

    fn build_setup(&self, c: CombatBuilder) -> CombatSetup {
        let mut notes = Vec::new();
        let local = self.ent(self.local_eid).cloned().unwrap_or_default();
        let teammate_pid = local.int("BACON_DUO_TEAMMATE_PLAYER_ID");
        let is_duos = teammate_pid > 0;

        // The local player.
        let hero = c.local_hero_tags.clone();
        let mut local_player = PlayerSetup {
            player_id: self.local_pid,
            hero_card_id: hero.card_id.clone(),
            hero_name: self.name_of(c.local_hero),
            tier: hero.int("PLAYER_TECH_LEVEL").max(1) as u32,
            hero_health: hero.health(),
            hero_armor: hero.int("ARMOR"),
            hero_power: c.local_hero_power.as_ref().map(|h| h.card_id.clone()),
            hero_power_start_of_combat: c
                .local_hero_power
                .as_ref()
                .is_some_and(|h| h.int("START_OF_COMBAT") > 0),
            hero_power_activated: c
                .local_hero_power
                .as_ref()
                .is_some_and(|h| h.int("BACON_HERO_POWER_ACTIVATED") > 0),
            trinkets: Vec::new(),
            trinket_numbers: BTreeMap::new(),
            deity: {
                let t = &c.local_tags;
                let get = |k: &str| t.get(k).and_then(|v| v.parse::<i32>().ok());
                get("BACON_OLD_GOD_ATTACK").zip(get("BACON_OLD_GOD_HEALTH"))
            },
            board: c.local_board.clone(),
            board_source: BoardSource::LocalBoard,
            hand: c.local_hand.clone(),
            pending_spells: c.local_pending.clone(),
            player_tags: c.local_tags.clone(),
            player_enchantments: c.local_enchantments.clone(),
            tags_known: true,
            fought: false,
            package_board: None,
        };
        // The local hero power and trinkets: entities controlled by us in PLAY.
        for (id, e) in &self.ents {
            if e.int("CONTROLLER") != self.local_pid || e.str("ZONE") != "PLAY" {
                continue;
            }
            match e.str("CARDTYPE") {
                "HERO_POWER" if local_player.hero_power.is_none() => {
                    local_player.hero_power = Some(e.card_id.clone());
                    local_player.hero_power_start_of_combat = e.int("START_OF_COMBAT") > 0;
                }
                "BATTLEGROUND_TRINKET" if e.int("BACON_IS_POTENTIAL_TRINKET") == 0 => {
                    let _ = id;
                    local_player.trinkets.push(e.card_id.clone());
                    local_player
                        .trinket_numbers
                        .insert(e.card_id.clone(), e.int("TAG_SCRIPT_DATA_NUM_1"));
                }
                _ => {}
            }
        }
        local_player.trinkets.sort();
        let copy: Vec<(String, i32, i32)> = c
            .local_copy
            .iter()
            .map(|&id| self.minion(id))
            .map(|m| (m.card_id, m.attack, m.health))
            .collect();
        let real: Vec<(String, i32, i32)> = c
            .local_board
            .iter()
            .map(|m| (m.card_id.clone(), m.attack, m.health))
            .collect();
        if !copy.is_empty() && copy != real {
            notes.push(format!(
                "the log's copy of the local board differs from the local board: {copy:?} vs {real:?}"
            ));
        }

        let mut players: Vec<PlayerSetup> = vec![local_player];
        for p in &c.packages {
            players.push(match &p.snapshot {
                Some(snap) => snap.clone(),
                None => {
                    notes.push(format!(
                        "player {}: its package has no board bracket",
                        p.player_id
                    ));
                    self.package_player(p)
                }
            });
        }

        // Entries: who fought, in which order, their player tags and entry boards.
        let mut order: Vec<i32> = Vec::new();
        for e in &c.entries {
            if !order.contains(&e.player_id) {
                order.push(e.player_id);
            }
            let Some(snap) = &e.snapshot else {
                continue;
            };
            let idx = match players.iter().position(|p| p.player_id == e.player_id) {
                Some(i) => i,
                None => {
                    // A warband without a package (Solo, or a missing package): build it from the
                    // entry.
                    let hero = snap
                        .hero
                        .and_then(|h| self.ent(h))
                        .cloned()
                        .unwrap_or_default();
                    players.push(PlayerSetup {
                        player_id: e.player_id,
                        hero_card_id: hero.card_id.clone(),
                        hero_name: snap.hero.map(|h| self.name_of(h)).unwrap_or_default(),
                        tier: hero.int("PLAYER_TECH_LEVEL").max(1) as u32,
                        hero_health: hero.health(),
                        hero_armor: hero.int("ARMOR"),
                        hero_power: None,
                        hero_power_start_of_combat: false,
                        hero_power_activated: false,
                        trinkets: Vec::new(),
                        trinket_numbers: BTreeMap::new(),
                        deity: None,
                        board: snap.board.clone(),
                        board_source: BoardSource::Entry,
                        hand: Vec::new(),
                        pending_spells: Vec::new(),
                        player_tags: BTreeMap::new(),
                        player_enchantments: Vec::new(),
                        tags_known: false,
                        fought: false,
                        package_board: None,
                    });
                    players.len() - 1
                }
            };
            let p = &mut players[idx];
            p.fought = true;
            // The pending spells its side showed when it entered (the teammate's and opponents'
            // are only in the log then).
            p.pending_spells = snap.pending.clone();
            if p.player_id != self.local_pid {
                p.player_tags = snap.tags.clone();
                p.player_enchantments = snap.enchantments.clone();
                p.tags_known = true;
                let get = |k: &str| snap.tags.get(k).and_then(|v| v.parse::<i32>().ok());
                if let Some(tier) = get("PLAYER_TECH_LEVEL").filter(|&t| t > 0) {
                    if tier as u32 != p.tier {
                        notes.push(format!(
                            "player {} Tavern Tier: package {} vs player tags {tier}",
                            p.player_id, p.tier
                        ));
                    }
                }
                if let Some(d) = get("BACON_OLD_GOD_ATTACK").zip(get("BACON_OLD_GOD_HEALTH")) {
                    if p.deity.is_some_and(|pd| pd != d) {
                        notes.push(format!(
                            "player {} Deity: package {:?} vs player tags at its entry {d:?} (the engine uses the latter)",
                            p.player_id, p.deity
                        ));
                    }
                    p.deity = Some(d);
                }
            }
            let entry_board = snap.board.clone();
            let same = |a: &[LogMinion], b: &[LogMinion]| {
                a.len() == b.len()
                    && a.iter().zip(b).all(|(x, y)| {
                        x.card_id == y.card_id && x.attack == y.attack && x.health == y.health
                    })
            };
            if p.board_source != BoardSource::Entry && !same(&p.board, &entry_board) {
                notes.push(format!(
                    "player {}: the board that entered the fight differs from its package board (the engine uses the former): [{}] vs [{}]",
                    p.player_id,
                    entry_board.iter().map(LogMinion::short).collect::<Vec<_>>().join(", "),
                    p.board.iter().map(LogMinion::short).collect::<Vec<_>>().join(", ")
                ));
            }
            match p.board_source {
                // What fought: the board that entered (before its Start of Combat).
                BoardSource::Package if !entry_board.is_empty() => {
                    p.package_board = Some(std::mem::replace(&mut p.board, entry_board));
                    p.board_source = BoardSource::Entry;
                }
                _ => {}
            }
        }

        let team_of = |pid: i32| pid == self.local_pid || (is_duos && pid == teammate_pid);
        let rank = |pid: i32| order.iter().position(|&p| p == pid).unwrap_or(usize::MAX);
        let mut a: Vec<PlayerSetup> = Vec::new();
        let mut b: Vec<PlayerSetup> = Vec::new();
        for p in players {
            if team_of(p.player_id) {
                a.push(p);
            } else {
                b.push(p);
            }
        }
        a.sort_by_key(|p| rank(p.player_id));
        b.sort_by_key(|p| rank(p.player_id));
        if b.is_empty() {
            notes.push("no enemy warband found".to_string());
        }

        let result = c.result.clone().unwrap_or(RealResult {
            outcome: BattleOutcome::Draw,
            damage: 0,
            winner_player_id: None,
            survivors: Vec::new(),
        });
        CombatSetup {
            turn: c.turn,
            is_duos,
            team_a: TeamSetup { players: a },
            team_b: TeamSetup { players: b },
            damage_cap: c.damage_cap,
            deity_dbid: c.deity_dbid.clone(),
            result,
            events: c.events,
            frames: c.frames,
            notes,
        }
    }
}

/// Reconstructs every combat of one game from its `GameState.DebugPrintPower()` lines (with the
/// prefix removed; `card_names` maps card IDs to names).
pub fn reconstruct_combats(
    lines: &[&str],
    card_names: &HashMap<String, String>,
) -> Vec<CombatSetup> {
    let mut t = Tracker::new(card_names);
    // The entity whose `tag=` lines follow, the indent of its header line, and whether it is
    // being created (`FULL_ENTITY`) rather than updated.
    let mut init: Option<(u32, usize, bool)> = None;
    for &raw in lines {
        let indent = raw.len() - raw.trim_start().len();
        let s = raw.trim();
        if let Some((eid, init_indent, created)) = init {
            if let Some(rest) = s.strip_prefix("tag=") {
                if indent > init_indent {
                    if let Some((k, v)) = rest.split_once(" value=") {
                        let e = t.ents.entry(eid).or_default();
                        e.tags.insert(k.trim().to_string(), v.trim().to_string());
                    }
                    continue;
                }
            }
            init = None;
            if created {
                t.on_created(eid);
            } else {
                t.on_shown(eid);
            }
        }

        if let Some(rest) = s.strip_prefix("GameEntity EntityID=") {
            if let Ok(eid) = rest.trim().parse() {
                t.game_eid = eid;
                t.ents.entry(eid).or_default().name = "GameEntity".into();
                init = Some((eid, indent, false));
            }
            continue;
        }
        if let Some(rest) = s.strip_prefix("Player EntityID=") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if let (Some(eid), Some(pid)) = (
                parts.first().and_then(|p| p.parse::<u32>().ok()),
                parts
                    .get(1)
                    .and_then(|p| p.strip_prefix("PlayerID="))
                    .and_then(|p| p.parse::<i32>().ok()),
            ) {
                if raw.contains("hi=0 lo=0") {
                    t.dummy_pid = pid;
                    t.dummy_eid = eid;
                } else {
                    t.local_pid = pid;
                    t.local_eid = eid;
                }
                let e = t.ents.entry(eid).or_default();
                e.tags.insert("PLAYER_ID".into(), pid.to_string());
                init = Some((eid, indent, false));
            }
            continue;
        }
        if let Some(rest) = s.strip_prefix("FULL_ENTITY - Creating ID=") {
            if let Some((id, cid)) = rest.split_once(" CardID=") {
                if let Ok(eid) = id.trim().parse::<u32>() {
                    let cid = cid.trim().to_string();
                    let name = t.card_names.get(&cid).cloned().unwrap_or_default();
                    t.ents.insert(
                        eid,
                        Ent {
                            card_id: cid,
                            name,
                            tags: HashMap::new(),
                        },
                    );
                    init = Some((eid, indent, true));
                }
            }
            continue;
        }
        if let Some(rest) = s
            .strip_prefix("SHOW_ENTITY - Updating Entity=")
            .or_else(|| s.strip_prefix("CHANGE_ENTITY - Updating Entity="))
        {
            if let Some((es, cid)) = rest.rsplit_once(" CardID=") {
                if let Some(eid) = t.resolve(es) {
                    let cid = cid.trim().to_string();
                    let name = t.card_names.get(&cid).cloned();
                    let e = t.ents.entry(eid).or_default();
                    if !cid.is_empty() {
                        e.card_id = cid;
                    }
                    if let Some(n) = name {
                        e.name = n;
                    }
                    init = Some((eid, indent, false));
                }
            }
            continue;
        }
        if s.starts_with("BLOCK_START") {
            t.on_block_start(indent, s);
            continue;
        }
        if s == "BLOCK_END" {
            t.on_block_end();
            continue;
        }
        if s.starts_with("META_DATA") {
            t.on_meta(s);
            continue;
        }
        if let Some(rest) = s
            .strip_prefix("TAG_CHANGE Entity=")
            .or_else(|| s.strip_prefix("HIDE_ENTITY - Entity="))
        {
            let Some(tag_pos) = rest.rfind(" tag=") else {
                continue;
            };
            let ent_str = &rest[..tag_pos];
            let Some((tag, value)) = rest[tag_pos + 5..].split_once(" value=") else {
                continue;
            };
            let (tag, value) = (tag.trim(), value.trim());
            if t.local_name.is_empty() && tag == "MULLIGAN_STATE" && !ent_str.starts_with('[') {
                t.local_name = ent_str.trim().to_string();
            }
            let Some(eid) = t.resolve(ent_str) else {
                continue;
            };
            let old = t
                .ents
                .entry(eid)
                .or_default()
                .tags
                .insert(tag.to_string(), value.to_string());
            t.on_tag(eid, tag, old, value);
        }
    }
    if let Some((eid, _, true)) = init {
        t.on_created(eid);
    }
    t.finish_combat();
    t.done
}

/// Every card a game's log shows, by card ID: `(name, CARDTYPE, Golden)`, for minions and spells
/// (to report what the engine doesn't implement).
pub fn cards_seen(
    lines: &[&str],
    card_names: &HashMap<String, String>,
) -> BTreeMap<String, (String, String, bool)> {
    let mut out = BTreeMap::new();
    let mut cur: Option<(String, usize)> = None;
    let mut ctype = String::new();
    let mut golden = false;
    let flush = |cid: &str,
                 ctype: &str,
                 golden: bool,
                 out: &mut BTreeMap<String, (String, String, bool)>| {
        if cid.is_empty() || !matches!(ctype, "MINION" | "SPELL" | "BATTLEGROUND_SPELL") {
            return;
        }
        let name = card_names.get(cid).cloned().unwrap_or_default();
        out.entry(cid.to_string())
            .or_insert((name, ctype.to_string(), golden));
    };
    for &raw in lines {
        let indent = raw.len() - raw.trim_start().len();
        let s = raw.trim();
        if let Some((cid, init_indent)) = &cur {
            if let Some(rest) = s.strip_prefix("tag=") {
                if indent > *init_indent {
                    if let Some((k, v)) = rest.split_once(" value=") {
                        match k.trim() {
                            "CARDTYPE" => ctype = v.trim().to_string(),
                            "PREMIUM" => golden = v.trim() != "0",
                            _ => {}
                        }
                    }
                    continue;
                }
            }
            flush(cid, &ctype, golden, &mut out);
            cur = None;
        }
        let cid = s
            .strip_prefix("FULL_ENTITY - Creating ID=")
            .and_then(|r| r.split_once(" CardID=").map(|(_, c)| c))
            .or_else(|| {
                s.strip_prefix("SHOW_ENTITY - Updating Entity=")
                    .or_else(|| s.strip_prefix("CHANGE_ENTITY - Updating Entity="))
                    .and_then(|r| r.rsplit_once(" CardID=").map(|(_, c)| c))
            });
        if let Some(cid) = cid {
            cur = Some((cid.trim().to_string(), indent));
            ctype.clear();
            golden = false;
        }
    }
    if let Some((cid, _)) = cur {
        flush(&cid, &ctype, golden, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal Duos combat: the local player (5) against player 7, whose package board (an
    /// Abyssal Envoy buffed to 6/4 after its creation) enters the fight; one attack, no hero
    /// attack (a draw).
    const LOG: &str = "CREATE_GAME
    GameEntity EntityID=1
        tag=ZONE value=PLAY
    Player EntityID=2 PlayerID=5 GameAccountId=[hi=1 lo=2]
        tag=BACON_DUO_TEAMMATE_PLAYER_ID value=6
        tag=TAVERN_SPELL_ATTACK_INCREASE value=2
    Player EntityID=3 PlayerID=10 GameAccountId=[hi=0 lo=0]
TAG_CHANGE Entity=GameEntity tag=TURN value=2 
FULL_ENTITY - Creating ID=20 CardID=TB_BaconShop_HERO_41
    tag=CONTROLLER value=5
    tag=CARDTYPE value=HERO
    tag=ZONE value=PLAY
    tag=PLAYER_TECH_LEVEL value=3
    tag=HEALTH value=30
TAG_CHANGE Entity=2 tag=HERO_ENTITY value=20 
FULL_ENTITY - Creating ID=21 CardID=BG36_108
    tag=CONTROLLER value=5
    tag=CARDTYPE value=MINION
    tag=ZONE value=PLAY
    tag=ZONE_POSITION value=1
    tag=ATK value=4
    tag=HEALTH value=5
    tag=TECH_LEVEL value=3
TAG_CHANGE Entity=GameEntity tag=BACON_IN_COMBAT_PHASE value=1 
    FULL_ENTITY - Creating ID=30 CardID=BG20_HERO_100
        tag=CONTROLLER value=5
        tag=CARDTYPE value=HERO
        tag=PLAYER_ID value=7
        tag=PLAYER_TECH_LEVEL value=4
        tag=HEALTH value=30
        tag=ZONE value=SETASIDE
    TAG_CHANGE Entity=30 tag=BACON_COMBAT_PHASE_HERO value=1 
    FULL_ENTITY - Creating ID=31 CardID=
        tag=ZONE value=SETASIDE
        tag=CONTROLLER value=5
    TAG_CHANGE Entity=GameEntity tag=1453 value=1 
    FULL_ENTITY - Creating ID=32 CardID=BG36_311
        tag=CONTROLLER value=5
        tag=CARDTYPE value=MINION
        tag=ZONE value=SETASIDE
        tag=ATK value=3
        tag=HEALTH value=4
        tag=TECH_LEVEL value=3
    TAG_CHANGE Entity=32 tag=ATK value=6 
    TAG_CHANGE Entity=GameEntity tag=1453 value=0 
    TAG_CHANGE Entity=GameEntity tag=1453 value=1 
    TAG_CHANGE Entity=Me#1234 tag=BACON_CURRENT_COMBAT_PLAYER_ID value=5 
    TAG_CHANGE Entity=Opponent tag=BACON_CURRENT_COMBAT_PLAYER_ID value=7 
    TAG_CHANGE Entity=Opponent tag=3874 value=2 
    FULL_ENTITY - Creating ID=40 CardID=BG36_311
        tag=CONTROLLER value=10
        tag=CARDTYPE value=MINION
        tag=ZONE value=PLAY
        tag=ZONE_POSITION value=1
        tag=ATK value=6
        tag=HEALTH value=4
        tag=TECH_LEVEL value=3
    TAG_CHANGE Entity=GameEntity tag=1453 value=0 
BLOCK_START BlockType=ATTACK Entity=21 EffectCardId=X EffectIndex=0 Target=0 SubOption=-1
    TAG_CHANGE Entity=GameEntity tag=PROPOSED_ATTACKER value=21 
    TAG_CHANGE Entity=GameEntity tag=PROPOSED_DEFENDER value=40 
    TAG_CHANGE Entity=40 tag=DAMAGE value=4 
    TAG_CHANGE Entity=21 tag=DAMAGE value=6 
BLOCK_END
BLOCK_START BlockType=DEATHS Entity=GameEntity EffectCardId=X EffectIndex=0 Target=0 SubOption=-1
    TAG_CHANGE Entity=40 tag=ZONE value=GRAVEYARD 
    TAG_CHANGE Entity=21 tag=ZONE value=GRAVEYARD 
BLOCK_END
TAG_CHANGE Entity=GameEntity tag=BACON_IN_COMBAT_PHASE value=0 
";

    #[test]
    fn reconstructs_a_duos_combat() {
        let lines: Vec<&str> = LOG.lines().collect();
        let combats = reconstruct_combats(&lines, &HashMap::new());
        assert_eq!(combats.len(), 1);
        let c = &combats[0];
        assert_eq!(c.turn, 1);
        assert!(c.is_duos);
        assert_eq!(c.result.outcome, BattleOutcome::Draw);

        let [me] = &c.team_a.players[..] else {
            panic!("team A: {:?}", c.team_a.players);
        };
        assert_eq!(
            (me.player_id, me.tier, me.board_source),
            (5, 3, BoardSource::LocalBoard)
        );
        assert_eq!(me.board.len(), 1);
        assert_eq!((me.board[0].attack, me.board[0].health), (4, 5));
        assert_eq!(
            me.player_tags
                .get("TAVERN_SPELL_ATTACK_INCREASE")
                .map(String::as_str),
            Some("2")
        );

        let [opp] = &c.team_b.players[..] else {
            panic!("team B: {:?}", c.team_b.players);
        };
        assert_eq!((opp.player_id, opp.tier), (7, 4));
        assert!(opp.fought && opp.tags_known);
        assert_eq!(opp.player_tags.get("3874").map(String::as_str), Some("2"));
        // The package's board, snapshotted when its bracket closed (after the buff), and the
        // board that entered the fight.
        assert_eq!(opp.board_source, BoardSource::Entry);
        assert_eq!(opp.board[0].card_id, "BG36_311");
        assert_eq!((opp.board[0].attack, opp.board[0].health), (6, 4));
        let package = opp.package_board.as_ref().unwrap();
        assert_eq!((package[0].attack, package[0].health), (6, 4));
        // The hidden card created before its board is a hidden hand card.
        assert_eq!(opp.hand.len(), 1);
        assert_eq!(opp.hand[0].card_id, None);

        let attacks: Vec<_> = c.attacks().collect();
        assert_eq!(attacks.len(), 1);
        let (side, att, def) = attacks[0];
        assert_eq!(*side, Side::A);
        assert_eq!((att.entity_id, att.attack, att.health), (21, 4, 5));
        assert_eq!((def.entity_id, def.attack, def.health), (40, 6, 4));
        let deaths = c
            .events
            .iter()
            .filter(|e| matches!(e, ServerEvent::Death { .. }))
            .count();
        // Frames: the warbands that entered, before the attack; then the boards once its deaths
        // resolved.
        let ids = |b: &[LogMinion]| b.iter().map(|m| m.entity_id).collect::<Vec<_>>();
        let [entered, end] = &c.frames[..] else {
            panic!("frames: {:?}", c.frames);
        };
        assert_eq!(
            (ids(&entered.board_a), ids(&entered.board_b)),
            (vec![21], vec![40])
        );
        assert!(matches!(
            c.events[entered.events],
            ServerEvent::Attack { .. }
        ));
        assert_eq!((end.board_a.len(), end.board_b.len()), (0, 0));
        assert_eq!(end.events, c.events.len());
        assert_eq!(deaths, 2);
    }
}
