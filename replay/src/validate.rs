//! Runs the engine on reconstructed combats ([`crate::reconstruct`]) and compares the results with
//! the real ones: the outcome and damage distribution over many seeds, and the first point where
//! the engine's attack sequence diverges from the server's.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

use seaglass::cards::{self, ballers, tokens, volumizers};
use seaglass::heroes;
use seaglass::model::{
    BattleOutcome, DeityKind, DeityState, EffectDuration, GameState, HeroPowerState, HeroState,
    PlayerAuras, PlayerEffect, Side, Tribe, TrinketState, Unit, UnitId,
};
use seaglass::scenario::card_index;
use seaglass::trinkets;
use seaglass::{resolve_battle, resolve_duos_battle, DuosTeam, Event, Rng, Warband};

use crate::cards::{engine_card, CardKind, MatchedBy};
use crate::reconstruct::{CombatSetup, HandCard, LogMinion, PlayerSetup, ServerEvent};

/// Cards whose combat effect reads their player's hand (`Flighty Scout` reads itself in hand).
pub const HAND_READERS: [&str; 6] = [
    "Choral Mrrrglr",
    "Diremuck Forager",
    "Costume Enthusiast",
    "Expert Aviator",
    "Deathly Striker",
    "Flighty Scout",
];

/// The enchantments a Hero Power puts on the minion it chose to act on at Start of Combat:
/// Teron Gorefiend's (`BG25_HERO_103pe`) and Flobbidinous Floop's (`BGDUO_HERO_101pe2`).
const MARKED_BY_HERO_POWER: [&str; 2] = ["BG25_HERO_103pe", "BGDUO_HERO_101pe2"];

/// The hero powers that act once per combat once unlocked (Turn 7), armed when the log shows
/// them active (`START_OF_COMBAT`): Vanndar's and Drek'Thar's.
const ARMED_WHEN_UNLOCKED: [&str; 2] = ["BG22_HERO_003p", "BG22_HERO_002p"];

/// `true` if the engine simulates the hero power `card_id` (any of its forms) in combat: it is an
/// implemented Hero Power with a combat hook (`engine/docs/heroes.md` §4).
pub fn hero_power_simulated(card_id: &str) -> bool {
    heroes::power_card(card_id).is_some_and(|id| {
        let h = cards::hooks(id);
        h.hero_start_of_combat.is_some()
            || h.hero_combat_space.is_some()
            || h.hero_enemy_killed.is_some()
    })
}

/// `true` if the engine simulates the trinket `card_id` in combat: it is an implemented trinket
/// with a combat hook (`engine/docs/trinkets.md` §6, §10).
pub fn trinket_simulated(card_id: &str) -> bool {
    trinkets::by_card_id(card_id).is_some_and(|t| {
        let h = cards::hooks(t.id);
        trinkets::is_implemented(t.id)
            && (h.trinket_start_of_combat.is_some()
                || h.trinket_deity_awakened.is_some()
                || h.trinket_friendly_death.is_some()
                || h.trinket_friendly_attack.is_some()
                || h.trinket_combat_summon.is_some())
    })
}

/// The player's trinkets for the engine (`engine/docs/trinkets.md` §10): the implemented ones
/// among those the log shows them owning, once each (a `Souvenir Stand` copy shows twice).
fn trinket_states(p: &PlayerSetup) -> Vec<TrinketState> {
    let mut states: Vec<TrinketState> = Vec::new();
    for card_id in &p.trinkets {
        let Some(t) = trinkets::by_card_id(card_id).filter(|t| trinkets::is_implemented(t.id))
        else {
            continue;
        };
        if !states.iter().any(|s| s.card_id == t.id) {
            states.push(trinkets::new_state(t.id));
        }
    }
    states
}

/// The player's hero for the engine (`engine/docs/heroes.md` §10): the hero of `p`'s hero card
/// (or skin), with the hero power the log shows (any form, even another hero's: Master Nguyen),
/// armed if it was used for this combat.
fn hero_state(p: &PlayerSetup) -> Option<HeroState> {
    let power = p.hero_power.as_deref().and_then(heroes::power_card)?;
    let hero = heroes::by_card_id(&p.hero_card_id);
    let mut state = match hero {
        Some(h) => heroes::new_state(h),
        None => HeroState {
            hero_id: 0,
            power: HeroPowerState {
                card_id: power,
                cost: 0,
                uses_left: 0,
                counter: 0,
                stacks: 0,
                armed: 0,
                stored: Vec::new(),
                picked: None,
            },
            second_power: None,
        },
    };
    state.power.card_id = power;
    state.power.uses_left = 0;
    // Armed for this combat: a power the player used for it (Y'Shaarj), or one that acts once per
    // combat once unlocked (Vanndar, Drek'Thar), which the log shows as `START_OF_COMBAT` on it.
    let unlocked = p.hero_power_start_of_combat
        && p.hero_power
            .as_deref()
            .is_some_and(|hp| ARMED_WHEN_UNLOCKED.contains(&hp));
    state.power.armed = u32::from(p.hero_power_activated || unlocked);
    Some(state)
}

/// A card of the reconstruction the engine doesn't implement, or maps only by name.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub struct CardNote {
    pub card_id: String,
    pub name: String,
}

/// The engine input built from a [`CombatSetup`].
#[derive(Clone, Debug)]
pub struct EngineInput {
    pub is_duos: bool,
    pub team_a: DuosTeam,
    pub team_b: DuosTeam,
    /// Each team's players' ids, in warband order (`team_a.warbands[i]` is `players_a[i]`).
    pub players_a: Vec<i32>,
    pub players_b: Vec<i32>,
    /// Minions (board or hand) the engine doesn't implement: simulated as vanilla bodies.
    pub unimplemented: Vec<CardNote>,
    /// Cards matched by name rather than by card ID.
    pub matched_by_name: Vec<CardNote>,
    /// What the reconstruction couldn't give the engine.
    pub gaps: Vec<String>,
}

fn tribe_of_race(race: &str) -> Tribe {
    match race {
        "BEAST" => Tribe::Beast,
        "DEMON" => Tribe::Demon,
        "DRAGON" => Tribe::Dragon,
        "ELEMENTAL" => Tribe::Elemental,
        "MECHANICAL" => Tribe::Mech,
        "MURLOC" => Tribe::Murloc,
        "PIRATE" => Tribe::Pirate,
        "QUILBOAR" => Tribe::Quilboar,
        "UNDEAD" => Tribe::Undead,
        "NAGA" => Tribe::Naga,
        "ABERRATION" => Tribe::Aberration,
        "ALL" => Tribe::All,
        _ => Tribe::None,
    }
}

struct Builder {
    unimplemented: Vec<CardNote>,
    matched_by_name: Vec<CardNote>,
}

impl Builder {
    /// The engine unit for a minion the log shows (its card, stats and keywords as shown).
    fn unit(&mut self, m: &LogMinion, auras: &PlayerAuras) -> Unit {
        let note = || CardNote {
            card_id: m.card_id.clone(),
            name: m.name.clone(),
        };
        let mapped = engine_card(&m.card_id, &m.name)
            .filter(|c| matches!(c.kind, CardKind::Minion | CardKind::Token | CardKind::Deity));
        let mut u = match &mapped {
            Some(c) => {
                if c.matched_by == MatchedBy::Name {
                    self.matched_by_name.push(note());
                }
                card_index()
                    .by_id(c.card_id)
                    .map(|card| card.proto.clone())
                    .unwrap_or_else(|| Unit::new(m.name.clone(), m.attack, m.health))
            }
            None => {
                self.unimplemented.push(note());
                let mut u = Unit::new(m.name.clone(), m.attack, m.health);
                u.tribe = tribe_of_race(&m.race);
                u
            }
        };
        if m.golden && !u.is_golden {
            u.make_golden();
        }
        u.tavern_tier = m.tier.max(1);
        u.taunt = m.taunt;
        u.divine_shield = m.divine_shield;
        u.windfury = m.windfury;
        u.reborn = m.reborn;
        u.venomous = m.venomous;
        u.stealth = m.stealth;
        // Chosen by its player's Hero Power for this Start of Combat (`engine/docs/heroes.md`
        // §3.3): Teron's "Impending Death", Floop's "In the Gloop".
        u.marked = m
            .enchantments
            .iter()
            .any(|e| MARKED_BY_HERO_POWER.contains(&e.as_str()));
        // The log's stats already include the "wherever this is" auras: mark them applied.
        cards::sync_unit_auras(&mut u, auras);
        u.attack = m.attack;
        u.health = m.health;
        u.max_attack = m.attack;
        u.max_health = m.health;
        u
    }

    fn hand_card(&mut self, c: &HandCard, auras: &PlayerAuras) -> Option<Unit> {
        if let Some(m) = &c.minion {
            return Some(self.unit(m, auras));
        }
        let id = c.card_id.as_deref()?;
        let card = engine_card(id, &c.name)?;
        if card.kind != CardKind::Spell {
            return None;
        }
        card_index().by_id(card.card_id).map(|c| c.proto.clone())
    }
}

/// Which Deity a game's `BACON_GLOBAL_OLD_GOD_DBID` / Deity minion card names.
pub fn deity_kind_of(card_or_dbid: &str) -> Option<DeityKind> {
    match card_or_dbid {
        "BGFYM_011" | "132532" => Some(DeityKind::YShaarj),
        "BGFYM_000" | "130610" => Some(DeityKind::CThun),
        _ => None,
    }
}

/// The player auras the log gives for `p` (see [`crate::reconstruct`] for the tags).
fn player_auras(p: &PlayerSetup, deity: DeityKind, gaps: &mut Vec<String>) -> PlayerAuras {
    let mut a = PlayerAuras::default();
    let tag = |k: &str| p.player_tags.get(k).and_then(|v| v.parse::<i32>().ok());
    a.deity = match p.deity {
        Some((attack, health)) => DeityState {
            kind: deity,
            attack,
            health,
            is_golden: false,
        },
        None => DeityState::new(deity),
    };
    a.hero_health = Some(p.hero_health);
    a.hero = hero_state(p);
    a.trinkets = trinket_states(p);
    if p.tags_known {
        a.spell_bonus_atk = tag("TAVERN_SPELL_ATTACK_INCREASE").unwrap_or(0);
        a.spell_bonus_hp = tag("TAVERN_SPELL_HEALTH_INCREASE").unwrap_or(0);
        // Tavern spells cast this game (`Faceless Converter`).
        a.tavern_spells_cast = tag("3088").unwrap_or(0).max(0) as u32;
        let beetle = (tag("3874").unwrap_or(0), tag("3924").unwrap_or(0));
        if beetle != (0, 0) {
            tokens::add_beetle_bonus(&mut a, beetle.0, beetle.1);
        }
    }
    for e in &p.player_enchantments {
        let [n1, n2] = e.script_data;
        match e.card_id.as_str() {
            // Nerubian Deathswarmer: "Your Undead have +N Attack this game".
            "BG25_011pe" => a.undead_bonus_attack = n1,
            // Volumizers: "+N1/+N2 this game".
            "BG34_170pe" => volumizers::add_volumizer_bonus(&mut a, n1, n2),
            // Ballers: future Ballers improved by N1.
            "BG31_816pe" => ballers::improve_ballers(&mut a, n1),
            // Beetle Army: the Beetle bonus (also player tags 3874/3924).
            "BG31_808pe" if !p.tags_known => tokens::add_beetle_bonus(&mut a, n1, n2),
            _ => {}
        }
    }
    // Spells waiting for this combat (`Brood of Nozdormu`, ...): their player effects.
    for c in &p.pending_spells {
        let card = c.card_id.as_deref().and_then(|id| engine_card(id, &c.name));
        let Some(card) = card.filter(|c| c.kind == CardKind::Spell) else {
            gaps.push(format!(
                "{} (player {}): pending spell {} is not implemented",
                p.hero_name, p.player_id, c.name
            ));
            continue;
        };
        match a.effects.iter_mut().find(|e| e.card_id == card.card_id) {
            Some(e) => e.stacks += 1,
            None => a.effects.push(PlayerEffect {
                card_id: card.card_id,
                stacks: 1,
                duration: EffectDuration::Turn,
            }),
        }
    }
    if !p.tags_known {
        gaps.push(format!(
            "{} (player {}): player tags unknown (Tavern spell bonus, Beetles)",
            p.hero_name, p.player_id
        ));
    }
    a
}

/// Builds the engine input for a reconstructed combat. `deity` is the game's Deity.
pub fn engine_input(setup: &CombatSetup, deity: DeityKind) -> EngineInput {
    let mut b = Builder {
        unimplemented: Vec::new(),
        matched_by_name: Vec::new(),
    };
    let mut gaps = Vec::new();
    let mut team = |players: &[PlayerSetup], gaps: &mut Vec<String>| -> (DuosTeam, Vec<i32>) {
        let mut warbands: Vec<Warband> = Vec::new();
        let mut ids = Vec::new();
        for p in players.iter().take(2) {
            let auras = player_auras(p, deity, gaps);
            let board = p.board.iter().map(|m| b.unit(m, &auras)).collect();
            let hand = p
                .hand
                .iter()
                .filter_map(|c| b.hand_card(c, &auras))
                .collect();
            let hidden = p.hand.iter().filter(|c| c.card_id.is_none()).count();
            if hidden > 0 {
                let readers: Vec<&str> = p
                    .board
                    .iter()
                    .map(|m| m.name.as_str())
                    .filter(|n| HAND_READERS.contains(n))
                    .collect();
                let note = if readers.is_empty() {
                    String::new()
                } else {
                    format!(" — it matters: {} read(s) the hand", readers.join(", "))
                };
                gaps.push(format!(
                    "{} (player {}): {hidden} hidden hand card(s){note}",
                    p.hero_name, p.player_id
                ));
            }
            for m in &p.board {
                if !m.dark_gifts.is_empty() {
                    gaps.push(format!(
                        "{} (player {}): {} has Dark Gift(s) {} (not simulated)",
                        p.hero_name,
                        p.player_id,
                        m.name,
                        m.dark_gifts.join(", ")
                    ));
                }
            }
            let simulated = p.hero_power.as_deref().is_some_and(hero_power_simulated);
            if p.hero_power_start_of_combat && !simulated {
                gaps.push(format!(
                    "{} (player {}): hero power {} has a Start of Combat effect (not simulated)",
                    p.hero_name,
                    p.player_id,
                    p.hero_power.as_deref().unwrap_or("?")
                ));
            }
            warbands.push(Warband {
                board,
                hand,
                auras,
                hero_tier: p.tier,
            });
            ids.push(p.player_id);
        }
        while warbands.len() < 2 {
            warbands.push(Warband::default());
        }
        let [w0, w1]: [Warband; 2] = warbands.try_into().ok().unwrap();
        (
            DuosTeam {
                warbands: [w0, w1],
                first: 0,
            },
            ids,
        )
    };
    let (team_a, players_a) = team(&setup.team_a.players, &mut gaps);
    let (team_b, players_b) = team(&setup.team_b.players, &mut gaps);
    let mut seen = HashSet::new();
    for e in &setup.events {
        if let ServerEvent::Trigger {
            side,
            source,
            card_id,
            kind,
            ..
        } = e
        {
            let what = match kind.as_str() {
                "HERO_POWER" if hero_power_simulated(card_id) => continue,
                "HERO_POWER" => "hero power",
                "BATTLEGROUND_TRINKET" if trinket_simulated(card_id) => continue,
                "BATTLEGROUND_TRINKET" => "trinket",
                _ => continue,
            };
            if seen.insert((*side, card_id.clone())) {
                gaps.push(format!(
                    "{what} {source} ({card_id}, side {side:?}) triggered in combat: not simulated"
                ));
            }
        }
    }
    let dedup = |v: Vec<CardNote>| {
        let mut v: Vec<CardNote> = v.into_iter().collect::<HashSet<_>>().into_iter().collect();
        v.sort();
        v
    };
    EngineInput {
        is_duos: setup.is_duos,
        team_a,
        team_b,
        players_a,
        players_b,
        unimplemented: dedup(b.unimplemented),
        matched_by_name: dedup(b.matched_by_name),
        gaps,
    }
}

// ---------------------------------------------------------------------------------------------
// Attack sequences
// ---------------------------------------------------------------------------------------------

/// One step of a combat, comparable between the server's log and the engine's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Step {
    /// A warband entered the fight.
    Entry { side: Side, player_id: i32 },
    /// An attack and both minions' stats when it was declared.
    Attack {
        side: Side,
        attacker: (String, i32, i32),
        defender: (String, i32, i32),
    },
}

impl Step {
    /// The same step ignoring stats.
    fn same_identity(&self, other: &Step) -> bool {
        match (self, other) {
            (
                Step::Entry {
                    side: s1,
                    player_id: p1,
                },
                Step::Entry {
                    side: s2,
                    player_id: p2,
                },
            ) => s1 == s2 && p1 == p2,
            (
                Step::Attack {
                    side: s1,
                    attacker: a1,
                    defender: d1,
                },
                Step::Attack {
                    side: s2,
                    attacker: a2,
                    defender: d2,
                },
            ) => s1 == s2 && a1.0 == a2.0 && d1.0 == d2.0,
            _ => false,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Step::Entry { side, player_id } => format!("{side:?}: player {player_id} enters"),
            Step::Attack {
                side,
                attacker,
                defender,
            } => format!(
                "{side:?}: {} {}/{} -> {} {}/{}",
                attacker.0, attacker.1, attacker.2, defender.0, defender.1, defender.2
            ),
        }
    }
}

/// A card name for comparisons: lowercase, without the engine's `Golden ` prefix, with ` ★` for
/// a Golden.
fn norm(name: &str, golden: bool) -> String {
    let n = name.trim();
    let n = n.strip_prefix("Golden ").unwrap_or(n);
    let star = if golden { " ★" } else { "" };
    format!("{}{star}", n.to_ascii_lowercase())
}

/// Sorts each run of consecutive entries by side (A first): warbands entering together (the
/// opening, simultaneous step-ins) are logged in either order.
fn normalize(steps: Vec<Step>) -> Vec<Step> {
    normalize_by(steps, |s| s)
}

fn normalize_by<T>(mut steps: Vec<T>, step: impl Fn(&T) -> &Step) -> Vec<T> {
    let mut i = 0;
    while i < steps.len() {
        let mut j = i;
        while j < steps.len() && matches!(step(&steps[j]), Step::Entry { .. }) {
            j += 1;
        }
        if j > i + 1 {
            steps[i..j].sort_by_key(|s| matches!(step(s), Step::Entry { side: Side::B, .. }));
        }
        i = j.max(i + 1);
    }
    steps
}

/// The server's steps. Warbands without minions "enter" in the log but never fight: skipped.
pub fn real_steps(setup: &CombatSetup) -> Vec<Step> {
    real_steps_indexed(setup)
        .into_iter()
        .map(|(s, _)| s)
        .collect()
}

/// [`real_steps`], each with the index of its event in `setup.events`.
pub fn real_steps_indexed(setup: &CombatSetup) -> Vec<(Step, usize)> {
    let empty: HashSet<i32> = setup
        .team_a
        .players
        .iter()
        .chain(&setup.team_b.players)
        .filter(|p| p.board.is_empty())
        .map(|p| p.player_id)
        .collect();
    setup
        .events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            ServerEvent::Entry { side, player_id }
                if setup.is_duos && !empty.contains(player_id) =>
            {
                Some((
                    Step::Entry {
                        side: *side,
                        player_id: *player_id,
                    },
                    i,
                ))
            }
            ServerEvent::Attack {
                side,
                attacker,
                defender,
            } => Some((
                Step::Attack {
                    side: *side,
                    attacker: (
                        norm(&attacker.name, attacker.golden),
                        attacker.attack,
                        attacker.health,
                    ),
                    defender: (
                        norm(&defender.name, defender.golden),
                        defender.attack,
                        defender.health,
                    ),
                },
                i,
            )),
            _ => None,
        })
        .collect::<Vec<_>>()
        .pipe(|v| normalize_by(v, |(s, _)| s))
}

/// `value.pipe(f)` = `f(value)`.
trait Pipe: Sized {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R {
        f(self)
    }
}

impl<T> Pipe for T {}

#[derive(Clone)]
struct Tracked {
    name: String,
    attack: i32,
    health: i32,
}

/// The engine's steps for one combat, replaying its event log over the starting units.
fn engine_steps(input: &EngineInput, events: &[Event]) -> Vec<Step> {
    let mut units: HashMap<UnitId, Tracked> = HashMap::new();
    // Ids: the first warbands (A's, then B's), then the second warbands (docs/duos.md §3.1),
    // each as `SideCombatState::new` prepares it; Solo: A's board, then B's.
    let mut next: UnitId = 0;
    let order: Vec<(Side, usize)> = if input.is_duos {
        vec![(Side::A, 0), (Side::B, 0), (Side::A, 1), (Side::B, 1)]
    } else {
        vec![(Side::A, 0), (Side::B, 0)]
    };
    for (side, w) in order {
        let team = if side == Side::A {
            &input.team_a
        } else {
            &input.team_b
        };
        let warband = &team.warbands[w];
        for u in warband.board.iter().take(7) {
            let mut u = u.clone();
            if let Some(combat_start) = cards::hooks(u.card_id).combat_start {
                combat_start(&mut u);
            }
            cards::sync_unit_auras(&mut u, &warband.auras);
            units.insert(
                next,
                Tracked {
                    name: norm(&u.name, u.is_golden),
                    attack: u.attack,
                    health: u.health,
                },
            );
            next += 1;
        }
    }
    let players = |side: Side| {
        if side == Side::A {
            &input.players_a
        } else {
            &input.players_b
        }
    };
    let mut fighting: HashMap<Side, usize> = HashMap::new();
    let mut steps = Vec::new();
    for e in events {
        match e {
            Event::WarbandSteppedIn { team, player, .. } => {
                fighting.insert(*team, *player);
                if input.is_duos {
                    steps.push(Step::Entry {
                        side: *team,
                        player_id: players(*team).get(*player).copied().unwrap_or(-1),
                    });
                }
            }
            Event::AttackDeclared {
                side,
                attacker,
                target,
            } => {
                let get = |id: &UnitId| {
                    units
                        .get(id)
                        .map(|t| (t.name.clone(), t.attack, t.health))
                        .unwrap_or_else(|| (format!("#{id}"), 0, 0))
                };
                steps.push(Step::Attack {
                    side: *side,
                    attacker: get(attacker),
                    defender: get(target),
                });
            }
            Event::StatBuff {
                unit,
                attack,
                health,
                ..
            } => {
                if let Some(t) = units.get_mut(unit) {
                    t.attack = *attack;
                    t.health = *health;
                }
            }
            Event::UnitSummoned {
                unit,
                name,
                attack,
                health,
                ..
            } => {
                units.insert(
                    *unit,
                    Tracked {
                        name: norm(name, name.starts_with("Golden ")),
                        attack: *attack,
                        health: *health,
                    },
                );
            }
            Event::DamageDealt { unit, amount, .. } => {
                if let Some(t) = units.get_mut(unit) {
                    t.health -= amount;
                }
            }
            Event::VenomousTriggered { target, .. } => {
                if let Some(t) = units.get_mut(target) {
                    t.health = t.health.min(0);
                }
            }
            Event::DeityAwakened {
                side, unit, name, ..
            } => {
                let team = if *side == Side::A {
                    &input.team_a
                } else {
                    &input.team_b
                };
                let w = fighting.get(side).copied().unwrap_or(0);
                let d = &team.warbands[w].auras.deity;
                units.insert(
                    *unit,
                    Tracked {
                        name: norm(name, d.is_golden),
                        attack: d.attack,
                        health: d.health,
                    },
                );
            }
            _ => {}
        }
    }
    normalize(steps)
}

// ---------------------------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------------------------

/// Where the engine's best-matching run first leaves the server's sequence.
#[derive(Clone, Debug, Serialize)]
pub struct Divergence {
    /// The seed of the best-matching run.
    pub seed: u64,
    /// Steps of the real sequence.
    pub real_len: usize,
    /// Leading steps whose attacker, defender and side match.
    pub matched: usize,
    /// Leading steps that also match on stats.
    pub matched_with_stats: usize,
    /// The first step whose stats differ (identity matching), as `(index, real, engine)`.
    pub first_stat_diff: Option<(usize, Step, Step)>,
    /// The first step whose identity differs, as `(index, real, engine)` (`None`: the
    /// sequence ended).
    pub first_diff: Option<(usize, Option<Step>, Option<Step>)>,
    /// What kind of divergence `first_diff` is.
    pub kind: String,
    /// The best run's outcome and damage (capped, side A's perspective).
    pub outcome: BattleOutcome,
    pub damage: i32,
}

/// The engine replaying the real attack targets ([`seaglass::combat::with_target_chooser`]):
/// every strike attacks the real defender, so the runs can only leave the real sequence where
/// the engine's rules or state differ (or another random effect does).
#[derive(Clone, Debug, Serialize)]
pub struct Guided {
    pub runs: u32,
    pub a_wins: u32,
    pub b_wins: u32,
    pub draws: u32,
    /// Runs with the real outcome, and with the real outcome and damage.
    pub real_outcome_runs: u32,
    pub real_exact_runs: u32,
    pub divergence: Option<Divergence>,
    /// Why the best run stopped following the real targets: `(real attack index, reason)`.
    pub lost: Option<(usize, String)>,
}

/// The engine's results on one combat, compared with the real result.
#[derive(Clone, Debug, Serialize)]
pub struct Validation {
    pub runs: u32,
    pub a_wins: u32,
    pub b_wins: u32,
    pub draws: u32,
    /// Signed capped damage (side A's perspective: `+` A won) → runs.
    pub damage: BTreeMap<i32, u32>,
    /// The real signed capped damage.
    pub real_damage: i32,
    /// Runs with the real outcome.
    pub real_outcome_runs: u32,
    /// Runs with the real outcome and damage.
    pub real_exact_runs: u32,
    /// Every run produced the same step sequence (no randomness).
    pub deterministic: bool,
    pub divergence: Option<Divergence>,
    pub guided: Option<Guided>,
    pub flags: Vec<String>,
}

fn signed(outcome: BattleOutcome, damage: u32, cap: Option<i32>) -> i32 {
    let d = damage as i32;
    let d = cap.map_or(d, |c| d.min(c));
    match outcome {
        BattleOutcome::AWin => d,
        BattleOutcome::BWin => -d,
        BattleOutcome::Draw => 0,
    }
}

/// One engine combat: `(outcome, hero damage, events)`.
fn run_engine(input: &EngineInput, seed: u64) -> (BattleOutcome, u32, Vec<Event>) {
    if input.is_duos {
        let r = resolve_duos_battle(&input.team_a, &input.team_b, seed);
        (r.outcome, r.hero_damage, r.events)
    } else {
        let [a, b] = [&input.team_a.warbands[0], &input.team_b.warbands[0]];
        let state = GameState {
            hero_tier_a: a.hero_tier,
            hero_tier_b: b.hero_tier,
            auras_a: a.auras.clone(),
            auras_b: b.auras.clone(),
            hand_a: a.hand.clone(),
            hand_b: b.hand.clone(),
        };
        let r = resolve_battle(&a.board, &b.board, &state, seed);
        (r.outcome, r.hero_damage, r.events)
    }
}

/// `(leading steps matching on identity, leading steps matching with stats)`.
fn prefix(real: &[Step], steps: &[Step]) -> (usize, usize) {
    let matched = real
        .iter()
        .zip(steps)
        .take_while(|(r, e)| r.same_identity(e))
        .count();
    let with_stats = real.iter().zip(steps).take_while(|(r, e)| r == e).count();
    (matched, with_stats)
}

fn divergence(
    real: &[Step],
    steps: &[Step],
    seed: u64,
    outcome: BattleOutcome,
    damage: i32,
) -> Divergence {
    let (matched, with_stats) = prefix(real, steps);
    let first_stat_diff = (with_stats < matched).then(|| {
        (
            with_stats,
            real[with_stats].clone(),
            steps[with_stats].clone(),
        )
    });
    let (first_diff, kind) = if matched < real.len() || matched < steps.len() {
        let r = real.get(matched).cloned();
        let e = steps.get(matched).cloned();
        let kind = match (&r, &e) {
            (Some(_), None) => "the engine's combat ended earlier".to_string(),
            (None, Some(_)) => "the real combat ended earlier".to_string(),
            (
                Some(Step::Attack {
                    side: rs,
                    attacker: ra,
                    defender: rd,
                }),
                Some(Step::Attack {
                    side: es,
                    attacker: ea,
                    defender: ed,
                }),
            ) => {
                if rs != es {
                    "a different side attacks".to_string()
                } else if ra.0 != ea.0 {
                    "a different attacker (attack order or board differs)".to_string()
                } else if rd.0 != ed.0 {
                    "a different target (random, or the boards differ)".to_string()
                } else {
                    "stats".to_string()
                }
            }
            (Some(Step::Entry { .. }), _) | (_, Some(Step::Entry { .. })) => {
                "a warband enters at a different point".to_string()
            }
            _ => String::new(),
        };
        (Some((matched, r, e)), kind)
    } else {
        (None, String::new())
    };
    Divergence {
        seed,
        real_len: real.len(),
        matched,
        matched_with_stats: with_stats,
        first_stat_diff,
        first_diff,
        kind,
        outcome,
        damage,
    }
}

/// A real attack, as the guided replay follows it.
#[derive(Clone)]
struct RealAttack {
    side: Side,
    attacker: String,
    defender: (String, i32, i32),
}

/// The guided replay's state: the real attacks, the next one, and where it lost them.
struct Guide {
    attacks: Vec<RealAttack>,
    next: usize,
    lost: Option<(usize, String)>,
}

/// The best guided run so far: its prefix lengths, divergence, and where it lost the targets.
type BestGuided = ((usize, usize), Divergence, Option<(usize, String)>);

/// One engine combat attacking the real targets while its attacks match the real ones.
fn guided_run(
    input: &EngineInput,
    attacks: &[RealAttack],
    seed: u64,
) -> (BattleOutcome, u32, Vec<Event>, Option<(usize, String)>) {
    let guide = std::rc::Rc::new(std::cell::RefCell::new(Guide {
        attacks: attacks.to_vec(),
        next: 0,
        lost: None,
    }));
    let g = guide.clone();
    let chooser = move |side: Side, attacker: &Unit, defenders: &[Unit], candidates: &[usize]| {
        let mut g = g.borrow_mut();
        if g.lost.is_some() {
            return None;
        }
        let k = g.next;
        g.next += 1;
        let att = norm(&attacker.name, attacker.is_golden);
        let Some(real) = g.attacks.get(k).cloned() else {
            g.lost = Some((
                k,
                format!(
                    "the engine attacks on ({side:?} {att}) after the real combat's last attack"
                ),
            ));
            return None;
        };
        if side != real.side || att != real.attacker {
            g.lost = Some((
                k,
                format!(
                    "the engine's attacker is {side:?} {att}, the real one {:?} {}",
                    real.side, real.attacker
                ),
            ));
            return None;
        }
        let pick = candidates
            .iter()
            .copied()
            .filter(|&i| norm(&defenders[i].name, defenders[i].is_golden) == real.defender.0)
            .min_by_key(|&i| {
                let d = &defenders[i];
                (
                    (d.attack - real.defender.1).abs() + (d.health - real.defender.2).abs(),
                    i,
                )
            });
        if pick.is_none() {
            let legal: Vec<String> = candidates
                .iter()
                .map(|&i| norm(&defenders[i].name, defenders[i].is_golden))
                .collect();
            g.lost = Some((
                k,
                format!(
                    "the real target {} isn't a legal target in the engine (candidates: {})",
                    real.defender.0,
                    legal.join(", ")
                ),
            ));
        }
        pick
    };
    let (outcome, damage, events) =
        seaglass::combat::with_target_chooser(chooser, || run_engine(input, seed));
    let lost = guide.borrow().lost.clone();
    (outcome, damage, events, lost)
}

/// Runs the engine `runs` times (seeds derived from `base_seed` as `simulate_batch` does) and
/// compares with the real result; then `guided_runs` runs replaying the real targets.
pub fn validate(
    setup: &CombatSetup,
    input: &EngineInput,
    runs: u32,
    guided_runs: u32,
    base_seed: u64,
) -> Validation {
    let real = real_steps(setup);
    let real_damage = match setup.result.outcome {
        BattleOutcome::AWin => setup.result.damage,
        BattleOutcome::BWin => -setup.result.damage,
        BattleOutcome::Draw => 0,
    };
    let mut seeder = Rng::new(base_seed);
    let mut v = Validation {
        runs,
        a_wins: 0,
        b_wins: 0,
        draws: 0,
        damage: BTreeMap::new(),
        real_damage,
        real_outcome_runs: 0,
        real_exact_runs: 0,
        deterministic: true,
        divergence: None,
        guided: None,
        flags: Vec::new(),
    };
    let mut first_identity: Option<Vec<Step>> = None;
    let mut best: Option<((usize, usize), Divergence)> = None;
    for _ in 0..runs {
        let seed = seeder.next_u64();
        let (outcome, damage, events) = run_engine(input, seed);
        let d = signed(outcome, damage, setup.damage_cap);
        match outcome {
            BattleOutcome::AWin => v.a_wins += 1,
            BattleOutcome::BWin => v.b_wins += 1,
            BattleOutcome::Draw => v.draws += 1,
        }
        *v.damage.entry(d).or_default() += 1;
        if outcome == setup.result.outcome {
            v.real_outcome_runs += 1;
            if d == real_damage {
                v.real_exact_runs += 1;
            }
        }
        let steps = engine_steps(input, &events);
        match &first_identity {
            None => first_identity = Some(steps.clone()),
            Some(first) => {
                if v.deterministic
                    && (first.len() != steps.len()
                        || first.iter().zip(&steps).any(|(a, b)| !a.same_identity(b)))
                {
                    v.deterministic = false;
                }
            }
        }
        let p = prefix(&real, &steps);
        if best.as_ref().is_none_or(|(bp, _)| p > *bp) {
            best = Some((p, divergence(&real, &steps, seed, outcome, d)));
        }
    }
    v.divergence = best.map(|(_, d)| d);

    if guided_runs > 0 {
        let attacks: Vec<RealAttack> = real
            .iter()
            .filter_map(|s| match s {
                Step::Attack {
                    side,
                    attacker,
                    defender,
                } => Some(RealAttack {
                    side: *side,
                    attacker: attacker.0.clone(),
                    defender: defender.clone(),
                }),
                Step::Entry { .. } => None,
            })
            .collect();
        let mut g = Guided {
            runs: guided_runs,
            a_wins: 0,
            b_wins: 0,
            draws: 0,
            real_outcome_runs: 0,
            real_exact_runs: 0,
            divergence: None,
            lost: None,
        };
        let mut best: Option<BestGuided> = None;
        let mut seeder = Rng::new(base_seed ^ 0x9e37_79b9_7f4a_7c15);
        for _ in 0..guided_runs {
            let seed = seeder.next_u64();
            let (outcome, damage, events, lost) = guided_run(input, &attacks, seed);
            let d = signed(outcome, damage, setup.damage_cap);
            match outcome {
                BattleOutcome::AWin => g.a_wins += 1,
                BattleOutcome::BWin => g.b_wins += 1,
                BattleOutcome::Draw => g.draws += 1,
            }
            if outcome == setup.result.outcome {
                g.real_outcome_runs += 1;
                if d == real_damage {
                    g.real_exact_runs += 1;
                }
            }
            let steps = engine_steps(input, &events);
            let p = prefix(&real, &steps);
            if best.as_ref().is_none_or(|(bp, ..)| p > *bp) {
                best = Some((p, divergence(&real, &steps, seed, outcome, d), lost));
            }
        }
        if let Some((_, d, lost)) = best {
            g.divergence = Some(d);
            g.lost = lost;
        }
        v.guided = Some(g);
    }

    let p = v.real_outcome_runs as f64 / runs.max(1) as f64;
    if v.real_outcome_runs == 0 {
        v.flags
            .push("the engine never produces the real outcome".to_string());
    } else if p < 0.02 {
        v.flags.push(format!(
            "the engine rarely produces the real outcome ({:.2}%)",
            p * 100.0
        ));
    }
    if v.real_outcome_runs > 0
        && setup.result.outcome != BattleOutcome::Draw
        && v.real_exact_runs == 0
    {
        v.flags.push(format!(
            "the engine never produces the real damage ({})",
            setup.result.damage
        ));
    }
    v
}
