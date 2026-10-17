//! Why a combat disagrees with the engine: the likely cause of its first divergence.
//!
//! The examined divergence is the guided replay's (the engine attacking the real targets, so
//! random targeting can't explain it), else the best free run's. Its *window* is the server's
//! events from the last step that matched exactly (side, attacker, defender and stats) through
//! the diverging step: what made the engine differ acted there, or earlier on a unit of that
//! step. The evidence, by cause:
//!
//! 1. **Patch drift** ([`crate::drift`]): a Deity awakening before the divergence when the Deity
//!    rules differ (or a Deity in the engine's diverging step); a drifted card summoned before it
//!    (the engine summons from its own data); for a damage difference, a survivor summoned in
//!    combat whose Tier differs. Weak: a drifted card acting in the window (a card whose stats
//!    changed may have had its text changed too).
//! 2. **Not simulated**: a hero power, trinket or Dark Gift that triggered in the window, a minion
//!    the engine doesn't implement that triggered in it or was summoned, or a unit of the
//!    diverging step that one of those summoned. Weak: one that triggered before the window, or
//!    a trigger of a minion with Dark Gifts (it may be the gift's).
//! 3. **Reconstruction gap** (weak): a minion whose trigger changed a unit of the differing steps
//!    shows stored numbers (`TAG_SCRIPT_DATA`: counters, stored buffs) the engine input doesn't
//!    take; a reconstruction caveat.
//! 4. **Hidden information** (weak): hidden hand cards of a player whose board reads its hand.
//! 5. **Likely engine bug**: none of the above (weak evidence: a simulated card whose trigger
//!    changed a unit of the differing steps, to compare with the engine's).
//!
//! The verdict is the first cause with strong evidence, else with weak evidence; for a combat
//! flagged only for its damage, damage evidence comes first. It sorts combats for a human: the
//! evidence says what to look at.

use std::collections::{BTreeSet, HashMap};

use serde::Serialize;

use seaglass::model::BattleOutcome;

use crate::cards::{engine_card, CardKind, DEITY_HS_IDS};
use crate::drift::{engine_tier, DriftReport, DEITY_NAMES};
use crate::reconstruct::{CombatSetup, ServerEvent};
use crate::validate::{real_steps_indexed, EngineInput, Step, Validation};

/// The likely cause of a divergence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Cause {
    /// The log's patch differs from the engine's on a card or number involved.
    PatchDrift,
    /// A hero power, trinket, Dark Gift or unimplemented minion acted.
    NotSimulated,
    /// The reconstruction didn't give the engine something that mattered (a stored counter).
    Reconstruction,
    /// Cards the log hides matter.
    HiddenInfo,
    /// Nothing else explains it: the engine's rules likely differ.
    EngineBug,
}

impl Cause {
    pub const ALL: [Cause; 5] = [
        Cause::PatchDrift,
        Cause::NotSimulated,
        Cause::Reconstruction,
        Cause::HiddenInfo,
        Cause::EngineBug,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Cause::PatchDrift => "likely patch drift",
            Cause::NotSimulated => "not simulated (hero power, trinket, Dark Gift, missing card)",
            Cause::Reconstruction => "reconstruction gap",
            Cause::HiddenInfo => "hidden information",
            Cause::EngineBug => "likely engine bug",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Cause::PatchDrift => "drift",
            Cause::NotSimulated => "not simulated",
            Cause::Reconstruction => "reconstruction",
            Cause::HiddenInfo => "hidden",
            Cause::EngineBug => "engine",
        }
    }
}

/// One piece of evidence for a cause.
#[derive(Clone, Debug, Serialize)]
pub struct Evidence {
    pub cause: Cause,
    pub strong: bool,
    /// It explains the damage (not the divergence).
    pub damage: bool,
    pub what: String,
}

/// The likely cause of a combat's divergence, and the evidence.
#[derive(Clone, Debug, Serialize)]
pub struct Diagnosis {
    /// The engine's best run differs from the real sequence (or the damage differs).
    pub diverged: bool,
    pub cause: Cause,
    /// Other causes with strong evidence.
    pub also: Vec<Cause>,
    /// The divergence examined.
    pub examined: String,
    /// Strong evidence first.
    pub evidence: Vec<Evidence>,
}

#[derive(Default)]
struct Collector {
    evidence: Vec<Evidence>,
    seen: BTreeSet<String>,
}

impl Collector {
    fn add(&mut self, cause: Cause, strong: bool, what: String) {
        self.push(cause, strong, false, what);
    }

    fn push(&mut self, cause: Cause, strong: bool, damage: bool, what: String) {
        if !self.seen.insert(what.clone()) {
            // Seen as weak, now strong: upgrade.
            if strong {
                if let Some(e) = self.evidence.iter_mut().find(|e| e.what == what) {
                    e.strong = true;
                }
            }
            return;
        }
        self.evidence.push(Evidence {
            cause,
            strong,
            damage,
            what,
        });
    }
}

fn implemented(card_id: &str, name: &str) -> bool {
    engine_card(card_id, name).is_some_and(|c| c.kind != CardKind::Spell)
}

fn is_deity(card_id: &str, name: &str) -> bool {
    DEITY_HS_IDS.iter().any(|(id, _)| *id == card_id)
        || DEITY_NAMES.contains(&name.to_ascii_lowercase().as_str())
}

/// What a trigger source of `kind` is when the engine doesn't simulate it.
fn not_simulated(kind: &str, card_id: &str, name: &str) -> Option<&'static str> {
    if card_id == "BG_OldGod" {
        return None;
    }
    match kind {
        "HERO_POWER" if !crate::validate::hero_power_simulated(card_id) => Some("hero power"),
        "HERO_POWER" => None,
        "BATTLEGROUND_TRINKET" if !crate::validate::trinket_simulated(card_id) => Some("trinket"),
        "BATTLEGROUND_TRINKET" => None,
        "ENCHANTMENT" => Some("Dark Gift"),
        "MINION" if !implemented(card_id, name) => Some("unimplemented minion"),
        "SPELL" | "BATTLEGROUND_SPELL"
            if engine_card(card_id, name).is_none_or(|c| c.kind != CardKind::Spell) =>
        {
            Some("unimplemented spell")
        }
        _ => None,
    }
}

fn step_names(step: &Step) -> Vec<String> {
    match step {
        Step::Attack {
            attacker, defender, ..
        } => vec![attacker.0.clone(), defender.0.clone()],
        Step::Entry { .. } => Vec::new(),
    }
}

/// Diagnoses a combat's divergence from the engine (`v` from [`crate::validate::validate`]).
pub fn diagnose(
    setup: &CombatSetup,
    input: &EngineInput,
    v: &Validation,
    drift: &DriftReport,
) -> Diagnosis {
    let guided = v.guided.as_ref().and_then(|g| g.divergence.as_ref());
    let (d, which) = match guided {
        Some(d) => (Some(d), "guided replay"),
        None => (v.divergence.as_ref(), "best run"),
    };
    let steps = real_steps_indexed(setup);
    let n = setup.events.len();
    let mut c = Collector::default();
    // The units of the real steps that differ (the stats difference and the divergence): their
    // entities, and their names (a reborn or copied unit is a new entity).
    let mut diverging: BTreeSet<u32> = BTreeSet::new();
    let mut diverging_names: BTreeSet<String> = BTreeSet::new();
    // `which`: [attacker, defender] to include.
    let mut units_of = |step: Option<usize>, which: [bool; 2]| {
        if let Some(ServerEvent::Attack {
            attacker, defender, ..
        }) = step.and_then(|i| steps.get(i)).map(|s| &setup.events[s.1])
        {
            for (u, on) in [attacker, defender].into_iter().zip(which) {
                if on {
                    diverging.insert(u.entity_id);
                    diverging_names.insert(u.name.to_ascii_lowercase());
                }
            }
        }
    };

    // The window: after the last exactly matching step, through the diverging step.
    let (start, end, examined) = match d {
        Some(d) => {
            let start = d
                .matched_with_stats
                .checked_sub(1)
                .and_then(|i| steps.get(i))
                .map_or(0, |s| s.1 + 1);
            let at = if d.first_diff.is_some() {
                d.matched
            } else {
                d.matched_with_stats
            };
            let end = steps.get(at).map_or(n, |s| s.1 + 1);
            // The stats difference: only the units whose stats differ.
            if let Some((i, real, eng)) = &d.first_stat_diff {
                let which = match (real, eng) {
                    (
                        Step::Attack {
                            attacker: ra,
                            defender: rd,
                            ..
                        },
                        Step::Attack {
                            attacker: ea,
                            defender: ed,
                            ..
                        },
                    ) => [ra != ea, rd != ed],
                    _ => [true, true],
                };
                units_of(Some(*i), which);
            }
            units_of(d.first_diff.as_ref().map(|(i, ..)| *i), [true, true]);
            let examined = match (&d.first_diff, &d.first_stat_diff) {
                (Some((i, ..)), _) => {
                    format!("{which}: first divergence at step {} ({})", i + 1, d.kind)
                }
                (None, Some((i, ..))) => {
                    format!("{which}: first stats difference at step {}", i + 1)
                }
                (None, None) => format!("{which}: every step matches"),
            };
            // A Deity in the engine's diverging step, when the Deity rules differ.
            let engine_steps = [
                d.first_diff.as_ref().and_then(|(_, _, e)| e.clone()),
                d.first_stat_diff.as_ref().map(|(_, _, e)| e.clone()),
            ];
            for step in engine_steps.iter().flatten() {
                for name in step_names(step) {
                    let name = name.trim_end_matches(" ★");
                    if DEITY_NAMES.contains(&name) && !drift.rules.is_empty() {
                        c.add(
                            Cause::PatchDrift,
                            true,
                            format!(
                                "the engine's diverging step involves {name}, and {}",
                                drift.rules[0]
                            ),
                        );
                    }
                }
            }
            (start, end, examined)
        }
        None => (0, n, "no engine run".to_string()),
    };

    // Starting minions showing stored numbers (`TAG_SCRIPT_DATA_NUM_1/2`: counters, stored buffs).
    let stored: HashMap<u32, [i32; 2]> = setup
        .team_a
        .players
        .iter()
        .chain(&setup.team_b.players)
        .flat_map(|p| p.board.iter())
        .filter(|m| m.script_data != [0, 0])
        .map(|m| (m.entity_id, m.script_data))
        .collect();
    let gifts: HashMap<u32, (&str, &[String])> = setup
        .team_a
        .players
        .iter()
        .chain(&setup.team_b.players)
        .flat_map(|p| p.board.iter())
        .filter(|m| {
            !m.dark_gifts.is_empty() && !crate::validate::dark_gifts_simulated(&m.enchantments)
        })
        .map(|m| (m.entity_id, (m.name.as_str(), m.dark_gifts.as_slice())))
        .collect();
    for (i, e) in setup.events.iter().enumerate().take(end) {
        let w = i >= start;
        let when = if w { "in the window" } else { "earlier" };
        match e {
            ServerEvent::Trigger {
                entity_id,
                source,
                card_id,
                kind,
                targets,
                ..
            } => {
                if let Some(what) = not_simulated(kind, card_id, source) {
                    let hit: BTreeSet<&str> = targets
                        .iter()
                        .filter(|(id, name)| {
                            diverging.contains(id)
                                || diverging_names.contains(&name.to_ascii_lowercase())
                        })
                        .map(|(_, name)| name.as_str())
                        .collect();
                    if hit.is_empty() {
                        c.add(
                            Cause::NotSimulated,
                            w,
                            format!("{what} {source} triggered {when}"),
                        );
                    } else {
                        c.add(
                            Cause::NotSimulated,
                            true,
                            format!(
                                "{what} {source} changed {} ({when}), a unit of the steps that differ",
                                hit.into_iter().collect::<Vec<_>>().join(", ")
                            ),
                        );
                    }
                } else if let Some((name, g)) = gifts.get(entity_id).filter(|_| w) {
                    c.add(
                        Cause::NotSimulated,
                        false,
                        format!(
                            "{name} (Dark Gift(s) {}) triggered in the window: maybe the gift",
                            g.join(", ")
                        ),
                    );
                }
                if w && kind == "MINION" && card_id != "BG_OldGod" {
                    let hit: BTreeSet<&str> = targets
                        .iter()
                        .filter(|(id, name)| {
                            diverging.contains(id)
                                || diverging_names.contains(&name.to_ascii_lowercase())
                        })
                        .map(|(_, name)| name.as_str())
                        .collect();
                    let hit = hit.into_iter().collect::<Vec<_>>().join(", ");
                    if let Some([a, b]) = stored.get(entity_id).filter(|_| !hit.is_empty()) {
                        c.add(
                            Cause::Reconstruction,
                            false,
                            format!(
                                "{source} changed {hit} and shows stored numbers [{a}/{b}] (TAG_SCRIPT_DATA) the engine input doesn't take: if its effect scales with them, the engine's differs"
                            ),
                        );
                    }
                    match drift.find(source) {
                        Some(why) if !hit.is_empty() => c.add(
                            Cause::PatchDrift,
                            true,
                            format!(
                                "drifted card {source} changed {hit}, a unit of the steps that differ ({why})"
                            ),
                        ),
                        Some(why) => c.add(
                            Cause::PatchDrift,
                            false,
                            format!("drifted card acts in the window: {why}"),
                        ),
                        None if !hit.is_empty() && implemented(card_id, source) => c.add(
                            Cause::EngineBug,
                            false,
                            format!(
                                "{source} changed {hit}, a unit of the steps that differ: compare the engine's {source}"
                            ),
                        ),
                        None => {}
                    }
                }
            }
            ServerEvent::Summon { unit, by, .. } => {
                let key = diverging.contains(&unit.entity_id)
                    || diverging_names.contains(&unit.name.to_ascii_lowercase());
                if is_deity(&unit.card_id, &unit.name) && !drift.rules.is_empty() {
                    c.add(
                        Cause::PatchDrift,
                        true,
                        format!(
                            "{} awakened before the divergence, and {}",
                            unit.name, drift.rules[0]
                        ),
                    );
                } else if let Some(why) = drift.find(&unit.name) {
                    c.add(
                        Cause::PatchDrift,
                        true,
                        format!(
                            "summoned before the divergence (the engine uses its own data): {why}"
                        ),
                    );
                }
                if !implemented(&unit.card_id, &unit.name) {
                    c.add(
                        Cause::NotSimulated,
                        true,
                        format!("{} (not implemented) was summoned {when}", unit.name),
                    );
                }
                let by_what = by
                    .as_ref()
                    .and_then(|b| Some((not_simulated(&b.kind, &b.card_id, &b.name)?, b)));
                if let Some((what, b)) = by_what {
                    let role = if key {
                        "a unit of the steps that differ"
                    } else {
                        when
                    };
                    c.add(
                        Cause::NotSimulated,
                        w || key,
                        format!("{what} {} summoned {} ({role})", b.name, unit.name),
                    );
                }
            }
            ServerEvent::Attack {
                attacker, defender, ..
            } if w => {
                for u in [attacker, defender] {
                    if let Some(why) = drift.find(&u.name) {
                        c.add(
                            Cause::PatchDrift,
                            false,
                            format!("drifted card acts in the window: {why}"),
                        );
                    }
                }
            }
            _ => {}
        }
    }

    // A damage difference with the real outcome: survivors summoned in combat (the engine gives
    // them its own Tier; board minions keep the log's).
    if v.real_outcome_runs > 0
        && v.real_exact_runs == 0
        && setup.result.outcome != BattleOutcome::Draw
    {
        let on_boards: BTreeSet<u32> = setup
            .team_a
            .players
            .iter()
            .chain(&setup.team_b.players)
            .flat_map(|p| p.board.iter().map(|m| m.entity_id))
            .collect();
        let mut alt = setup.result.damage;
        let mut changed = Vec::new();
        for m in &setup.result.survivors {
            if on_boards.contains(&m.entity_id) {
                continue;
            }
            if let Some(t) = engine_tier(&m.card_id, &m.name).filter(|&t| t != m.tier) {
                alt += t as i32 - m.tier as i32;
                changed.push(format!(
                    "{} (Tier {} in the log, {t} in the engine)",
                    m.name, m.tier
                ));
            }
        }
        if !changed.is_empty() {
            let alt = setup.damage_cap.map_or(alt, |cap| alt.min(cap));
            let signed = if setup.result.outcome == BattleOutcome::AWin {
                alt
            } else {
                -alt
            };
            let runs = *v.damage.get(&signed).unwrap_or(&0);
            c.push(
                Cause::PatchDrift,
                true,
                true,
                format!(
                    "survivors summoned in combat: {}; with the engine's Tiers the real damage would be {alt} (the engine deals it in {:.1}% of runs)",
                    changed.join(", "),
                    100.0 * runs as f64 / v.runs.max(1) as f64
                ),
            );
        }
    }

    for g in &input.gaps {
        if g.contains("read(s) the hand") {
            c.add(Cause::HiddenInfo, false, g.clone());
        }
    }
    for note in &setup.notes {
        c.add(Cause::Reconstruction, false, note.clone());
    }

    let mut evidence = c.evidence;
    evidence.sort_by_key(|e| (!e.strong, e.cause));
    // Flagged only for the damage: damage evidence first.
    let outcome_ok = v.real_outcome_runs as f64 >= 0.02 * v.runs.max(1) as f64;
    let order: [bool; 2] = if outcome_ok {
        [true, false]
    } else {
        [false, true]
    };
    let cause = order
        .iter()
        .find_map(|&damage| {
            let mut of = evidence.iter().filter(|e| e.damage == damage);
            of.clone().find(|e| e.strong).or_else(|| of.next())
        })
        .map_or(Cause::EngineBug, |e| e.cause);
    let also: Vec<Cause> = evidence
        .iter()
        .filter(|e| e.strong && e.cause != cause)
        .map(|e| e.cause)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let diverged = d.is_some_and(|d| d.first_diff.is_some() || d.first_stat_diff.is_some())
        || (v.real_outcome_runs > 0
            && v.real_exact_runs == 0
            && setup.result.outcome != BattleOutcome::Draw);
    Diagnosis {
        diverged,
        cause,
        also,
        examined,
        evidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconstruct::{
        BoardSource, LogMinion, PlayerSetup, RealResult, Source, TeamSetup, UnitRef,
    };
    use crate::validate::{engine_input, Divergence, Guided};
    use seaglass::model::{DeityKind, Side};

    fn unit(entity_id: u32, card_id: &str, name: &str) -> UnitRef {
        UnitRef {
            entity_id,
            card_id: card_id.into(),
            name: name.into(),
            golden: false,
            attack: 1,
            health: 3,
        }
    }

    fn player(player_id: i32, board: Vec<LogMinion>) -> PlayerSetup {
        PlayerSetup {
            player_id,
            hero_card_id: String::new(),
            hero_name: format!("player {player_id}"),
            tier: 1,
            hero_health: 30,
            hero_armor: 0,
            hero_power: None,
            hero_power_start_of_combat: false,
            hero_power_activated: false,
            trinkets: Vec::new(),
            deity: None,
            board,
            board_source: BoardSource::LocalBoard,
            hand: Vec::new(),
            pending_spells: Vec::new(),
            player_tags: Default::default(),
            player_enchantments: Vec::new(),
            tags_known: true,
            fought: true,
            package_board: None,
        }
    }

    fn weaver(entity_id: u32) -> LogMinion {
        LogMinion {
            entity_id,
            card_id: "BGS_004".into(),
            name: "Wrath Weaver".into(),
            attack: 1,
            health: 3,
            tier: 1,
            golden: false,
            race: "DEMON".into(),
            taunt: false,
            divine_shield: false,
            windfury: false,
            reborn: false,
            venomous: false,
            stealth: false,
            enchantments: Vec::new(),
            dark_gifts: Vec::new(),
            script_data: [0, 0],
        }
    }

    /// A Solo combat: A's Wrath Weaver (10) attacks B's (20), then `middle` happens, then B's
    /// `attacker` attacks A's Wrath Weaver; the engine's best guided run matches the first attack
    /// and then B's Wrath Weaver attacks instead.
    fn diagnose_with(
        middle: Vec<ServerEvent>,
        attacker: UnitRef,
        drift: &DriftReport,
    ) -> Diagnosis {
        let mut events = vec![ServerEvent::Attack {
            side: Side::A,
            attacker: unit(10, "BGS_004", "Wrath Weaver"),
            defender: unit(20, "BGS_004", "Wrath Weaver"),
        }];
        events.extend(middle);
        events.push(ServerEvent::Attack {
            side: Side::B,
            attacker,
            defender: unit(10, "BGS_004", "Wrath Weaver"),
        });
        let setup = CombatSetup {
            turn: 1,
            is_duos: false,
            team_a: TeamSetup {
                players: vec![player(1, vec![weaver(10)])],
            },
            team_b: TeamSetup {
                players: vec![player(2, vec![weaver(20)])],
            },
            damage_cap: None,
            deity_dbid: String::new(),
            result: RealResult {
                outcome: BattleOutcome::BWin,
                damage: 2,
                winner_player_id: Some(2),
                survivors: Vec::new(),
            },
            events,
            notes: Vec::new(),
        };
        let real = crate::validate::real_steps(&setup);
        let engine_step = Step::Attack {
            side: Side::B,
            attacker: ("wrath weaver".into(), 1, 3),
            defender: ("wrath weaver".into(), 1, 3),
        };
        let divergence = Divergence {
            seed: 1,
            real_len: real.len(),
            matched: 1,
            matched_with_stats: 1,
            first_stat_diff: None,
            first_diff: Some((1, Some(real[1].clone()), Some(engine_step))),
            kind: "a different attacker (attack order or board differs)".into(),
            outcome: BattleOutcome::Draw,
            damage: 0,
        };
        let v = Validation {
            runs: 10,
            a_wins: 0,
            b_wins: 0,
            draws: 10,
            damage: [(0, 10)].into_iter().collect(),
            real_damage: -2,
            real_outcome_runs: 0,
            real_exact_runs: 0,
            deterministic: true,
            divergence: Some(divergence.clone()),
            guided: Some(Guided {
                runs: 10,
                a_wins: 0,
                b_wins: 0,
                draws: 10,
                real_outcome_runs: 0,
                real_exact_runs: 0,
                divergence: Some(divergence),
                lost: None,
            }),
            flags: vec!["the engine never produces the real outcome".into()],
        };
        diagnose(&setup, &engine_input(&setup, DeityKind::YShaarj), &v, drift)
    }

    #[test]
    fn a_trinket_summoning_the_attacker_is_not_simulated() {
        let trinket = Source {
            entity_id: 50,
            card_id: "BG36_MagicItem_407".into(),
            name: "Dragonhead Staff".into(),
            kind: "BATTLEGROUND_TRINKET".into(),
        };
        let summoned = unit(30, "BGS_004", "Wrath Weaver");
        let d = diagnose_with(
            vec![ServerEvent::Summon {
                side: Side::B,
                unit: summoned.clone(),
                by: Some(trinket),
            }],
            summoned,
            &DriftReport::default(),
        );
        assert_eq!(d.cause, Cause::NotSimulated, "{d:?}");
        assert!(d.evidence[0].strong);
        assert!(d.evidence[0].what.contains("Dragonhead Staff"), "{d:?}");
    }

    #[test]
    fn a_deity_awakening_under_drifted_rules_is_patch_drift() {
        let old_god = Source {
            entity_id: 60,
            card_id: "BG_OldGod".into(),
            name: "Secret Deity [DNT]".into(),
            kind: "SPELL".into(),
        };
        let deity = unit(31, "BGFYM_011", "Y'Shaarj");
        let drift = DriftReport {
            rules: vec!["the Deity awakens after 3 friendly Aberration deaths vs engine 4".into()],
            ..Default::default()
        };
        let d = diagnose_with(
            vec![ServerEvent::Summon {
                side: Side::B,
                unit: deity.clone(),
                by: Some(old_god),
            }],
            deity,
            &drift,
        );
        assert_eq!(d.cause, Cause::PatchDrift, "{d:?}");
        assert!(
            d.evidence.iter().all(|e| e.cause == Cause::PatchDrift),
            "{d:?}"
        );
    }

    #[test]
    fn an_unexplained_divergence_is_an_engine_bug() {
        let d = diagnose_with(
            Vec::new(),
            unit(20, "BGS_004", "Wrath Weaver"),
            &DriftReport::default(),
        );
        assert_eq!(d.cause, Cause::EngineBug, "{d:?}");
        assert!(d.evidence.is_empty());
    }
}
