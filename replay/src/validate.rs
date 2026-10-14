//! Runs the engine on reconstructed combats ([`crate::reconstruct`]) and compares the results with
//! the real ones: the outcome and damage distribution over many seeds, and the first point where
//! the engine's attack sequence diverges from the server's.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;

use seaglass::cards::{self, ballers, tokens, volumizers};
use seaglass::model::{
    BattleOutcome, DeityKind, DeityState, GameState, PlayerAuras, Side, Tribe, Unit, UnitId,
};
use seaglass::scenario::card_index;
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
    if p.tags_known {
        a.spell_bonus_atk = tag("TAVERN_SPELL_ATTACK_INCREASE").unwrap_or(0);
        a.spell_bonus_hp = tag("TAVERN_SPELL_HEALTH_INCREASE").unwrap_or(0);
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
            if p.hero_power_start_of_combat {
                gaps.push(format!(
                    "{} (player {}): hero power {} has a Start of Combat effect (hero powers are not simulated)",
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
                "HERO_POWER" => "hero power",
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
fn normalize(mut steps: Vec<Step>) -> Vec<Step> {
    let mut i = 0;
    while i < steps.len() {
        let mut j = i;
        while j < steps.len() && matches!(steps[j], Step::Entry { .. }) {
            j += 1;
        }
        if j > i + 1 {
            steps[i..j].sort_by_key(|s| match s {
                Step::Entry { side, .. } => *side == Side::B,
                _ => false,
            });
        }
        i = j.max(i + 1);
    }
    steps
}

/// The server's steps. Warbands without minions "enter" in the log but never fight: skipped.
pub fn real_steps(setup: &CombatSetup) -> Vec<Step> {
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
        .filter_map(|e| match e {
            ServerEvent::Entry { side, player_id }
                if setup.is_duos && !empty.contains(player_id) =>
            {
                Some(Step::Entry {
                    side: *side,
                    player_id: *player_id,
                })
            }
            ServerEvent::Attack {
                side,
                attacker,
                defender,
            } => Some(Step::Attack {
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
            }),
            _ => None,
        })
        .collect::<Vec<_>>()
        .pipe(normalize)
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

/// Runs the engine `runs` times (seeds derived from `base_seed` as `simulate_batch` does) and
/// compares with the real result.
pub fn validate(setup: &CombatSetup, input: &EngineInput, runs: u32, base_seed: u64) -> Validation {
    let real = real_steps(setup);
    let real_damage = match setup.result.outcome {
        BattleOutcome::AWin => setup.result.damage,
        BattleOutcome::BWin => -setup.result.damage,
        BattleOutcome::Draw => 0,
    };
    let solo_state = GameState {
        hero_tier_a: input.team_a.warbands[0].hero_tier,
        hero_tier_b: input.team_b.warbands[0].hero_tier,
        auras_a: input.team_a.warbands[0].auras.clone(),
        auras_b: input.team_b.warbands[0].auras.clone(),
        hand_a: input.team_a.warbands[0].hand.clone(),
        hand_b: input.team_b.warbands[0].hand.clone(),
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
        flags: Vec::new(),
    };
    let mut first_identity: Option<Vec<Step>> = None;
    let mut best: Option<(usize, usize, u64, Vec<Step>, BattleOutcome, i32)> = None;
    for _ in 0..runs {
        let seed = seeder.next_u64();
        let (outcome, damage, events) = if input.is_duos {
            let r = resolve_duos_battle(&input.team_a, &input.team_b, seed);
            (r.outcome, r.hero_damage, r.events)
        } else {
            let r = resolve_battle(
                &input.team_a.warbands[0].board,
                &input.team_b.warbands[0].board,
                &solo_state,
                seed,
            );
            (r.outcome, r.hero_damage, r.events)
        };
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
        let matched = real
            .iter()
            .zip(&steps)
            .take_while(|(r, e)| r.same_identity(e))
            .count();
        let with_stats = real.iter().zip(&steps).take_while(|(r, e)| r == e).count();
        let better = match &best {
            None => true,
            Some((m, w, ..)) => (matched, with_stats) > (*m, *w),
        };
        if better {
            best = Some((matched, with_stats, seed, steps, outcome, d));
        }
    }
    if let Some((matched, with_stats, seed, steps, outcome, damage)) = best {
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
        v.divergence = Some(Divergence {
            seed,
            real_len: real.len(),
            matched,
            matched_with_stats: with_stats,
            first_stat_diff,
            first_diff,
            kind,
            outcome,
            damage,
        });
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
