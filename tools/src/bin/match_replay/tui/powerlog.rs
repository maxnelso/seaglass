//! A Hearthstone `Power.log` game as a replay player source (`match_replay tui Power.log`).
//!
//! The game comes from the `seaglass_replay` parser: the local player's Tavern after every action
//! (`TavernPhaseReplay::snapshots`) and every combat with both boards before each attack
//! (`CombatSetup::frames`). The log shows nothing of the teammate's Tavern phase, only their
//! warband at each combat, so that is what the Tavern view shows of them.
//!
//! Each combat also carries the engine's version of it ([`CombatPlayback::alt`]): the engine
//! re-runs the combat from the warbands that entered it, attacking the real targets while its
//! attacks match the real ones (`validate::engine_trace`), and the screen flags the first strike
//! after which the boards differ.

use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use seaglass::{full_catalog, CardTemplate, CombatSnapshot, DeityKind, Event, Side, Tribe, Unit};
use seaglass_replay::reconstruct::{CombatSetup, LogMinion, PlayerSetup, ServerEvent};
use seaglass_replay::validate::{deity_kind_of, engine_input, engine_trace};
use seaglass_replay::{
    map_hs_card, parse_power_log_file, GameReplay, ReplayCard, ReplayUnit, TavernActionRecord,
    TavernSnapshot,
};
use seaglass_tools::fmt::CombatNames;

use super::combat::{engine_steps, ids, CombatPlayback, Phase, Step};
use super::show::{fresh, FrameInfo, Kind, Row, Source, TavernView};

/// Whether `path` looks like a `Power.log` rather than a seaglass replay.
pub fn is_power_log(path: &Path) -> bool {
    use std::io::Read;
    let mut head = vec![0; 1 << 16];
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let n = file.read(&mut head).unwrap_or(0);
    String::from_utf8_lossy(&head[..n]).contains("GameState.Debug")
}

/// The engine's cards by id.
struct Cards(HashMap<seaglass::CardId, CardTemplate>);

impl Cards {
    fn new() -> Self {
        Self(full_catalog().into_iter().map(|t| (t.card_id, t)).collect())
    }

    /// A unit for a card the log shows: the engine's card when it knows it (for its tribe,
    /// badges and text), else a plain body.
    fn unit(
        &self,
        hs_id: &str,
        name: &str,
        attack: i32,
        health: i32,
        tier: u32,
        golden: bool,
    ) -> Unit {
        let template = map_hs_card(hs_id, name).and_then(|(id, _, _)| self.0.get(&id));
        let mut unit = match template {
            Some(t) => t.instantiate(),
            None => {
                let mut u = Unit::new(name, attack, health);
                u.card_id = 0;
                u
            }
        };
        let mult = if golden && !unit.is_golden { 2 } else { 1 };
        if template.is_some() {
            unit.base_attack *= mult;
            unit.base_health *= mult;
        } else {
            unit.base_attack = attack;
            unit.base_health = health;
        }
        // The log's name, unless it never named the card (then the parser gives its card ID):
        // then the engine's name for it.
        if !name.is_empty() && name != hs_id {
            unit.name = name.to_string();
        } else if template.is_none() {
            unit.name = seaglass_replay::cards::engine_card(hs_id, name)
                .map(|c| {
                    c.name
                        .strip_prefix("Golden ")
                        .unwrap_or(&c.name)
                        .to_string()
                })
                .unwrap_or_else(|| name.to_string());
        }
        unit.attack = attack;
        unit.health = health;
        unit.max_attack = attack;
        unit.max_health = health;
        unit.tavern_tier = tier.max(1);
        unit.is_golden = golden;
        unit
    }

    fn log_unit(&self, m: &LogMinion) -> Unit {
        let mut u = self.unit(&m.card_id, &m.name, m.attack, m.health, m.tier, m.golden);
        u.id = m.entity_id;
        if u.card_id == 0 {
            u.tribe = tribe_of_race(&m.race);
        }
        u.taunt = m.taunt;
        u.divine_shield = m.divine_shield;
        u.windfury = m.windfury;
        u.reborn = m.reborn;
        u.venomous = m.venomous;
        u.stealth = m.stealth;
        u
    }

    fn board_unit(&self, r: &ReplayUnit) -> Unit {
        let mut u = self.unit(&r.card_id, &r.name, r.attack, r.health, r.tier, r.is_golden);
        u.id = r.entity_id;
        let has = |k: &str| r.keywords.iter().any(|x| x == k);
        u.taunt = has("Taunt");
        u.divine_shield = has("Divine Shield");
        u.windfury = has("Windfury");
        u.reborn = has("Reborn");
        u.venomous = has("Venomous");
        u.stealth = has("Stealth");
        u.magnetic = has("Magnetic");
        u
    }

    fn card_unit(&self, c: &ReplayCard) -> Unit {
        if c.card_type != "MINION" {
            let mut u = Unit::new(&c.name, 0, 0);
            u.id = c.entity_id;
            u.card_id = 0;
            u.is_spell = true;
            u.spell_cost = c.cost.unwrap_or(0).max(0) as u32;
            u.tavern_tier = c.tier.unwrap_or(1);
            return u;
        }
        let (attack, health) = c
            .stats
            .as_deref()
            .and_then(|s| s.split_once('/'))
            .and_then(|(a, h)| Some((a.parse().ok()?, h.parse().ok()?)))
            .unwrap_or((0, 0));
        let mut u = self.unit(
            &c.card_id,
            &c.name,
            attack,
            health,
            c.tier.unwrap_or(1),
            c.is_golden,
        );
        u.id = c.entity_id;
        u
    }
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

struct LogFrame {
    info: FrameInfo,
    title: String,
    tavern: TavernView,
    combat: Option<CombatPlayback>,
}

/// One game of a `Power.log`.
pub struct PowerLogSource {
    names: Vec<String>,
    frames: Vec<LogFrame>,
}

/// Load game `game` (1-based; default: the last) of a `Power.log`. Returns the source and a
/// line describing the game.
pub fn load(path: &Path, game: Option<usize>) -> Result<(PowerLogSource, String), String> {
    let games = parse_power_log_file(path)?;
    if games.is_empty() {
        return Err(format!("{}: no Battlegrounds game found", path.display()));
    }
    let index = game.unwrap_or(games.len());
    let g = games.get(index.wrapping_sub(1)).ok_or_else(|| {
        format!(
            "game {index} does not exist (the log has {} games)",
            games.len()
        )
    })?;
    let label = format!(
        "game {index}/{} · {} · {} · {}",
        games.len(),
        if g.is_duos { "Duos" } else { "Solo" },
        g.start_time,
        g.hero_name
    );
    Ok((PowerLogSource::new(g), label))
}

impl PowerLogSource {
    fn new(g: &GameReplay) -> Self {
        let cards = Cards::new();
        let player = g
            .player_name
            .split('#')
            .next()
            .unwrap_or(&g.player_name)
            .to_string();
        let deity = g
            .combats
            .first()
            .and_then(|c| deity_kind_of(&c.deity_dbid))
            .unwrap_or(DeityKind::CThun);
        let mut frames = Vec::new();
        let mut teammate: Option<(String, Vec<Unit>, u32)> = None;
        for turn in &g.turns {
            let t = &turn.tavern;
            let setup = g.combats.iter().find(|c| c.turn == turn.turn);
            let view = |snap: &TavernSnapshot,
                        before: Option<&TavernSnapshot>,
                        mate: &Option<(String, Vec<Unit>, u32)>| {
                tavern_view(&cards, &player, &g.hero_name, g.is_duos, snap, before, mate)
            };
            let start = TavernSnapshot {
                gold: t.starting_gold,
                tavern_tier: t.tavern_tier,
                upgrade_cost: t.upgrade_cost,
                hero_health: t.hero_health,
                hero_armor: t.hero_armor,
                board: t.initial_board.clone(),
                hand: t.initial_hand.clone(),
                shop: t.initial_shop.clone(),
                is_frozen: false,
            };
            frames.push(LogFrame {
                info: FrameInfo {
                    turn: turn.turn,
                    kind: Kind::TurnStart,
                    actor: Some(0),
                },
                title: format!("{player} starts turn {} · {}g", turn.turn, t.starting_gold),
                tavern: view(&start, None, &teammate),
                combat: None,
            });
            let mut previous = start;
            for (i, action) in t.actions.iter().enumerate() {
                let snap = t
                    .snapshots
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| previous.clone());
                frames.push(LogFrame {
                    info: FrameInfo {
                        turn: turn.turn,
                        kind: Kind::Act,
                        actor: Some(0),
                    },
                    title: format!("{player}: {}", describe_action(action)),
                    tavern: view(&snap, Some(&previous), &teammate),
                    combat: None,
                });
                previous = snap;
            }
            let Some(setup) = setup else {
                continue;
            };
            // The teammate's warband as it entered this combat.
            if let Some(mate) =
                setup.team_a.players.iter().find(|p| {
                    p.board_source != seaglass_replay::reconstruct::BoardSource::LocalBoard
                })
            {
                teammate = Some((
                    hero_name(mate),
                    mate.board.iter().map(|m| cards.log_unit(m)).collect(),
                    turn.turn,
                ));
            }
            let combat = log_combat(&cards, setup, &player, deity);
            frames.push(LogFrame {
                info: FrameInfo {
                    turn: turn.turn,
                    kind: Kind::Combat,
                    actor: None,
                },
                title: format!("combat vs {}: {}", opponents(setup), combat.result),
                tavern: view(&previous, None, &teammate),
                combat: Some(combat),
            });
        }
        if let Some(last) = frames.last() {
            frames.push(LogFrame {
                info: FrameInfo {
                    turn: last.info.turn,
                    kind: Kind::End,
                    actor: None,
                },
                title: "end of the log".to_string(),
                tavern: last.tavern.clone(),
                combat: None,
            });
        }
        Self {
            names: vec![player],
            frames,
        }
    }
}

/// The enemy team's heroes.
fn opponents(setup: &CombatSetup) -> String {
    let names: Vec<String> = setup.team_b.players.iter().map(hero_name).collect();
    names.join(" + ")
}

fn describe_action(action: &TavernActionRecord) -> String {
    let on = |target: &Option<String>| {
        target
            .as_ref()
            .map_or(String::new(), |t| format!(" on {t}"))
    };
    match action {
        TavernActionRecord::BuyMinion {
            card, gold_after, ..
        } => format!("Buy {card} · {gold_after}g left"),
        TavernActionRecord::BuySpell { spell, gold_after } => {
            format!("Buy {spell} · {gold_after}g left")
        }
        TavernActionRecord::PlayMinion { card, target, .. } => format!("Play {card}{}", on(target)),
        TavernActionRecord::CastSpell { spell, target, .. } => {
            format!("Cast {spell}{}", on(target))
        }
        TavernActionRecord::SellMinion { card, gold_after } => {
            format!("Sell {card} · {gold_after}g")
        }
        TavernActionRecord::Refresh { gold_after, .. } => format!("Refresh · {gold_after}g left"),
        TavernActionRecord::Freeze { is_frozen } => {
            if *is_frozen { "Freeze" } else { "Unfreeze" }.to_string()
        }
        TavernActionRecord::UpgradeTier {
            new_tier,
            gold_after,
        } => {
            format!("Upgrade to Tier {new_tier} · {gold_after}g left")
        }
        TavernActionRecord::Activate { card, target, .. } => {
            format!("Activate {card}{}", on(target))
        }
        TavernActionRecord::HeroPower { card, target, .. } => {
            format!("Hero Power {card}{}", on(target))
        }
        TavernActionRecord::Triple { golden_card } => format!("Triple! {golden_card}"),
        TavernActionRecord::PassCard { card, .. } => format!("Pass {card} to the teammate"),
    }
}

#[allow(clippy::too_many_arguments)]
fn tavern_view(
    cards: &Cards,
    player: &str,
    hero: &str,
    duos: bool,
    snap: &TavernSnapshot,
    before: Option<&TavernSnapshot>,
    teammate: &Option<(String, Vec<Unit>, u32)>,
) -> TavernView {
    let armor = if snap.hero_armor > 0 {
        format!("+{}", snap.hero_armor)
    } else {
        String::new()
    };
    let upgrade = match snap.upgrade_cost {
        Some(c) if snap.tavern_tier < 6 => format!(" (up {c}g)"),
        _ => String::new(),
    };
    let hero_line = format!(
        "{player} · {hero} ♥{}{armor} · Tier {}{upgrade} · Gold {}",
        snap.hero_health, snap.tavern_tier, snap.gold
    );
    let shop: Vec<Unit> = snap.shop.iter().map(|c| cards.card_unit(c)).collect();
    let board: Vec<Unit> = snap.board.iter().map(|u| cards.board_unit(u)).collect();
    let hand: Vec<Unit> = snap.hand.iter().map(|c| cards.card_unit(c)).collect();
    let before_units = |f: &dyn Fn(&TavernSnapshot) -> Vec<Unit>| before.map(f);
    let shop_before = before_units(&|b| b.shop.iter().map(|c| cards.card_unit(c)).collect());
    let board_before = before_units(&|b| b.board.iter().map(|u| cards.board_unit(u)).collect());
    let hand_before = before_units(&|b| b.hand.iter().map(|c| cards.card_unit(c)).collect());
    let mut rows = vec![
        Row {
            label: if snap.is_frozen {
                "Shop ❄ frozen"
            } else {
                "Shop"
            }
            .to_string(),
            fresh: fresh(&shop, shop_before.as_deref()),
            units: shop,
            slots: 0,
        },
        Row {
            label: "Board".to_string(),
            fresh: fresh(&board, board_before.as_deref()),
            units: board,
            slots: 7,
        },
        Row {
            label: format!("Hand {}/10", snap.hand.len()),
            fresh: fresh(&hand, hand_before.as_deref()),
            units: hand,
            slots: 0,
        },
    ];
    let detail_line = match teammate {
        Some((name, units, turn)) => {
            rows.push(Row {
                label: format!("Teammate {name} · board as it entered combat on turn {turn}"),
                fresh: vec![false; units.len()],
                units: units.clone(),
                slots: 0,
            });
            format!("Duos · teammate {name} (the log shows their warband only at combats)")
        }
        None if duos => "Duos · the teammate's warband shows from the first combat on".to_string(),
        None => "Solo".to_string(),
    };
    TavernView {
        hero_line,
        detail_line,
        rows,
        offers: Vec::new(),
    }
}

/// `Your team wins · 5 damage`, `The opponents win · 5 damage`.
fn team_result(outcome: seaglass::BattleOutcome, damage: u32) -> String {
    match outcome {
        seaglass::BattleOutcome::AWin => format!("Your team wins · {damage} damage"),
        seaglass::BattleOutcome::BWin => format!("The opponents win · {damage} damage"),
        seaglass::BattleOutcome::Draw => "Draw".to_string(),
    }
}

/// `Name ★ 5/6`.
fn unit_text(name: &str, golden: bool, attack: i32, health: i32) -> String {
    let star = if golden { " ★" } else { "" };
    format!("{name}{star} {attack}/{health}")
}

/// A player of a combat by id: their setup and whether they are the local player.
fn player_of(setup: &CombatSetup, pid: i32) -> Option<&PlayerSetup> {
    setup
        .team_a
        .players
        .iter()
        .chain(&setup.team_b.players)
        .find(|p| p.player_id == pid)
}

fn player_label(p: &PlayerSetup, local: &str) -> String {
    let who = match p.board_source {
        seaglass_replay::reconstruct::BoardSource::LocalBoard => format!(" ({local})"),
        _ => String::new(),
    };
    let armor = if p.hero_armor > 0 {
        format!("+{}", p.hero_armor)
    } else {
        String::new()
    };
    format!(
        "{}{who} · Tier {} · ♥{}{armor}",
        hero_name(p),
        p.tier,
        p.hero_health
    )
}

/// A player's hero, by name (the engine's name if the log never named it).
fn hero_name(p: &PlayerSetup) -> String {
    if !p.hero_name.is_empty() && p.hero_name != p.hero_card_id {
        return p.hero_name.clone();
    }
    seaglass::heroes::by_card_id(&p.hero_card_id)
        .map_or_else(|| p.hero_card_id.clone(), |h| h.name.to_string())
}

/// A combat as the log shows it, with the engine's version attached.
fn log_combat(cards: &Cards, setup: &CombatSetup, local: &str, deity: DeityKind) -> CombatPlayback {
    // A forced attack (`Jailbird Juggernaut`'s Golem attacking the target first) is part of its
    // summoner's strike, as the engine resolves it: drop the frame between them.
    let frames: Vec<&seaglass_replay::reconstruct::CombatFrame> = setup
        .frames
        .iter()
        .enumerate()
        .filter(|&(k, _)| {
            let Some(next) = setup.frames.get(k + 1) else {
                return true;
            };
            let range = setup.frames[k].events..next.events;
            !range
                .clone()
                .any(|i| seaglass_replay::validate::is_forced_attack(setup, i))
        })
        .map(|(_, f)| f)
        .collect();
    let snapshots: Vec<CombatSnapshot> = frames
        .into_iter()
        .map(|f| CombatSnapshot {
            events: f.events,
            board_a: f.board_a.iter().map(|m| cards.log_unit(m)).collect(),
            board_b: f.board_b.iter().map(|m| cards.log_unit(m)).collect(),
        })
        .collect();
    let side_word = |side: &Side| match side {
        Side::A => "[us] ",
        Side::B => "[them] ",
    };
    let texts: Vec<String> = setup
        .events
        .iter()
        .map(|e| match e {
            ServerEvent::Entry { side, player_id } => {
                let name = player_of(setup, *player_id).map_or_else(|| "?".to_string(), hero_name);
                format!("{}{name}'s warband enters the fight", side_word(side))
            }
            ServerEvent::Trigger {
                side,
                source,
                targets,
                ..
            } => {
                let on = if targets.is_empty() {
                    String::new()
                } else {
                    let names: Vec<&str> = targets.iter().map(|(_, n)| n.as_str()).collect();
                    format!(" → {}", names.join(", "))
                };
                format!("{}{source} triggers{on}", side_word(side))
            }
            ServerEvent::Attack {
                side,
                attacker,
                defender,
            } => format!(
                "{}{} attacks {}",
                side_word(side),
                attacker.short(),
                defender.short()
            ),
            ServerEvent::Summon { side, unit, by } => {
                let by = by
                    .as_ref()
                    .map_or(String::new(), |s| format!(" by {}", s.name));
                format!("{}{} is summoned{by}", side_word(side), unit.short())
            }
            ServerEvent::Death { side, unit } => {
                format!(
                    "{}{} dies",
                    side_word(side),
                    unit_text(&unit.name, unit.golden, unit.attack, unit.health)
                )
            }
        })
        .collect();

    // Who fights on each side, step by step: the latest entries.
    let first = |players: &[PlayerSetup]| players.first().map(|p| p.player_id);
    let fighters_after = |end: usize| {
        let mut f = [first(&setup.team_a.players), first(&setup.team_b.players)];
        for e in &setup.events[..end.min(setup.events.len())] {
            if let ServerEvent::Entry { side, player_id } = e {
                f[(*side == Side::B) as usize] = Some(*player_id);
            }
        }
        f
    };
    let labels_after = |end: usize| {
        fighters_after(end).map(|pid| {
            pid.and_then(|pid| player_of(setup, pid))
                .map_or_else(|| "?".to_string(), |p| player_label(p, local))
        })
    };

    let mut steps = Vec::new();
    let mut strikes = 0;
    if !snapshots.is_empty() {
        steps.push(Step::plain(Phase::Start, 0, 0..snapshots[0].events, 0));
    }
    for k in 1..snapshots.len() {
        let (before, after) = (&snapshots[k - 1], &snapshots[k]);
        let range = before.events..after.events;
        let attack = setup.events[range.clone()].iter().find_map(|e| match e {
            ServerEvent::Attack {
                side,
                attacker,
                defender,
            } => Some((*side, attacker, defender)),
            _ => None,
        });
        let Some((side, attacker, defender)) = attack else {
            let entered = setup.events[range.clone()]
                .iter()
                .any(|e| matches!(e, ServerEvent::Entry { .. }));
            let phase = match (entered, strikes > 0) {
                (true, true) => Phase::TagIn,
                (false, true) => Phase::Settle,
                (_, false) => Phase::StartOfCombat,
            };
            steps.push(Step::plain(phase, k, range, strikes));
            continue;
        };
        let mut strike = Step::plain(Phase::Strike, k - 1, range.clone(), strikes);
        strike.attacker = Some((side, attacker.entity_id));
        strike.target = Some((side.other(), defender.entity_id));
        let alive_after: Vec<u32> = ids(after).collect();
        strike.dying = ids(before).filter(|id| !alive_after.contains(id)).collect();
        let units_after: HashMap<u32, &Unit> = after
            .board_a
            .iter()
            .chain(&after.board_b)
            .map(|u| (u.id, u))
            .collect();
        for u in before.board_a.iter().chain(&before.board_b) {
            match units_after.get(&u.id) {
                Some(now) => {
                    if now.health < u.health {
                        strike.damage.push((u.id, u.health - now.health));
                    }
                    if u.divine_shield && !now.divine_shield {
                        strike.popped.push(u.id);
                    }
                }
                // The attack's two minions hit each other; anything else that died, we can't
                // say by how much.
                None if u.id == defender.entity_id => strike.damage.push((u.id, attacker.attack)),
                None if u.id == attacker.entity_id => strike.damage.push((u.id, defender.attack)),
                None => {}
            }
        }
        strikes += 1;
        steps.push(strike);
        steps.push(Step::plain(Phase::Settle, k, range, strikes));
    }
    let last = snapshots.len().saturating_sub(1);
    steps.push(Step::plain(
        Phase::End,
        last,
        snapshots.get(last).map_or(0, |s| s.events)..setup.events.len(),
        strikes,
    ));
    for step in &mut steps {
        step.labels = labels_after(step.events.end);
    }

    let result = team_result(setup.result.outcome, setup.result.damage.max(0) as u32);
    let mut real = CombatPlayback {
        snapshots,
        texts,
        steps,
        players: [Some(0), None],
        result,
        version: "real",
        warning: None,
        alt: None,
    };
    if setup.frames.is_empty() {
        real.warning = Some("the log has no board snapshots for this combat".to_string());
        return real;
    }
    match panic::catch_unwind(AssertUnwindSafe(|| engine_combat(setup, local, deity))) {
        Ok(engine) => {
            let note = compare(&real, &engine);
            real.warning = note.clone();
            let mut engine = engine;
            if engine.warning.is_none() {
                engine.warning = note;
            }
            real.alt = Some(Box::new(engine));
        }
        Err(_) => real.warning = Some("the engine panicked on this combat".to_string()),
    }
    real
}

/// The seeds tried for the engine's version of a combat.
const ENGINE_SEEDS: u64 = 64;

/// The engine's version of a combat: the warbands that entered it, attacking the real targets
/// while its attacks match the real ones (of [`ENGINE_SEEDS`] runs, the one that follows them the
/// longest).
fn engine_combat(setup: &CombatSetup, local: &str, deity: DeityKind) -> CombatPlayback {
    let input = engine_input(setup, deity);
    // The run that follows the real attacks the longest (who attacks first on a tie, and the
    // random draws after the engine stops following, depend on the seed).
    let mut trace = engine_trace(setup, &input, 0);
    let followed = |t: &seaglass_replay::validate::EngineTrace| {
        t.lost.as_ref().map_or(usize::MAX, |(k, _)| *k)
    };
    for seed in 1..ENGINE_SEEDS {
        if trace.lost.is_none() {
            break;
        }
        let other = engine_trace(setup, &input, seed);
        if followed(&other) > followed(&trace) {
            trace = other;
        }
    }
    let mut units: Vec<(u32, Side, String)> = Vec::new();
    for s in &trace.snapshots {
        for (side, board) in [(Side::A, &s.board_a), (Side::B, &s.board_b)] {
            units.extend(board.iter().map(|u| (u.id, side, u.name.clone())));
        }
    }
    let mut names = CombatNames::from_units(["us", "them"], units);
    let texts: Vec<String> = trace.events.iter().map(|e| names.describe(e)).collect();
    let mut steps = engine_steps(&trace.snapshots, &trace.events);
    // Who fights: the first warbands, then whoever steps in.
    let pid = |team: usize, player: usize| {
        let players = if team == 0 {
            &input.players_a
        } else {
            &input.players_b
        };
        players.get(player).copied()
    };
    let first = [pid(0, input.team_a.first), pid(1, input.team_b.first)];
    for step in &mut steps {
        let mut f = first;
        for e in &trace.events[..step.events.end] {
            if let Event::WarbandSteppedIn { team, player, .. } = e {
                let t = (*team == Side::B) as usize;
                f[t] = pid(t, *player);
            }
        }
        step.labels = f.map(|pid| {
            pid.and_then(|pid| player_of(setup, pid))
                .map_or_else(|| "?".to_string(), |p| player_label(p, local))
        });
    }
    let mut warnings = Vec::new();
    if let Some((k, why)) = &trace.lost {
        warnings.push(format!(
            "the engine stops following the real attacks at strike {}: {why}",
            k + 1
        ));
    }
    if !input.unimplemented.is_empty() {
        let names: Vec<&str> = input
            .unimplemented
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        warnings.push(format!(
            "not in the engine (vanilla bodies): {}",
            names.join(", ")
        ));
    }
    let damage = match setup.damage_cap {
        Some(cap) => trace.damage.min(cap.max(0) as u32),
        None => trace.damage,
    };
    CombatPlayback {
        snapshots: trace.snapshots,
        texts,
        steps,
        players: [Some(0), None],
        result: team_result(trace.outcome, damage),
        version: "engine",
        warning: (!warnings.is_empty()).then(|| warnings.join(" · ")),
        alt: None,
    }
}

/// A board as the comparison sees it: each minion's name and stats, sorted.
type Board = Vec<(String, i32, i32)>;

/// Where the engine's version first differs from the real one: the first strike after which the
/// boards (names and stats) differ, or the result.
fn compare(real: &CombatPlayback, engine: &CombatPlayback) -> Option<String> {
    let strikes = |c: &CombatPlayback| c.steps.iter().filter(|s| s.phase == Phase::Strike).count();
    let (r, e) = (strikes(real), strikes(engine));
    if let Some(k) = (1..r.min(e)).find(|&k| boards_after(real, k) != boards_after(engine, k)) {
        return Some(format!(
            "⚠ the engine's boards differ from strike {k} on (e: engine)"
        ));
    }
    if real.result != engine.result {
        return Some(format!(
            "⚠ the engine's result differs: {} (e: engine)",
            engine.result
        ));
    }
    if r != e {
        return Some(format!(
            "⚠ the engine's combat has {e} strikes, the real one {r} (e: engine)"
        ));
    }
    None
}

/// The boards after strike `strike` of a playback, as they stand before the next strike (after
/// any warband stepping in), sorted per side. Names are compared without the engine's `Golden`
/// prefix (the log names a Golden minion like a normal one).
fn boards_after(c: &CombatPlayback, strike: usize) -> Option<[Board; 2]> {
    let step = c.steps.iter().rev().find(|s| {
        s.strike == strike && matches!(s.phase, Phase::Settle | Phase::TagIn | Phase::StartOfCombat)
    })?;
    let snap = &c.snapshots[step.snapshot];
    let side = |units: &[Unit]| {
        let mut v: Board = units
            .iter()
            .map(|u| {
                let name = u.name.strip_prefix("Golden ").unwrap_or(&u.name);
                (name.to_string(), u.attack, u.health)
            })
            .collect();
        v.sort();
        v
    };
    Some([side(&snap.board_a), side(&snap.board_b)])
}

/// The texts of strike `strike`'s events.
fn strike_texts(c: &CombatPlayback, strike: usize) -> Vec<String> {
    c.steps
        .iter()
        .find(|s| s.phase == Phase::Strike && s.strike + 1 == strike)
        .map(|s| c.texts[s.events.clone()].to_vec())
        .unwrap_or_default()
}

/// `match_replay compare`: every combat of a `Power.log` game next to the engine's version of
/// it: the results, and the first strike after which the boards differ, with the minions that
/// differ and both versions of that strike.
///
/// With `turn`, only that turn's combat; with `full`, also both versions step by step (each
/// step's events and the boards it shows).
pub fn compare_report(
    path: &Path,
    game: Option<usize>,
    turn: Option<u32>,
    full: bool,
) -> Result<String, String> {
    use std::fmt::Write;
    let (source, label) = load(path, game)?;
    let mut out = format!("{} · {label}\n", path.display());
    let (mut total, mut same, mut same_result) = (0, 0, 0);
    for i in 0..source.len() {
        if turn.is_some_and(|t| source.info(i).turn != t) {
            continue;
        }
        let Some(real) = source.combat(i) else {
            continue;
        };
        if full {
            for c in std::iter::once(&real).chain(real.alt.as_deref()) {
                let _ = writeln!(
                    out,
                    "\n--- {} version, turn {}",
                    c.version,
                    source.info(i).turn
                );
                for step in &c.steps {
                    let _ = writeln!(
                        out,
                        "  [{:?} {}] {} | {}",
                        step.phase, step.strike, step.labels[0], step.labels[1]
                    );
                    for t in &c.texts[step.events.clone()] {
                        let _ = writeln!(out, "      {t}");
                    }
                    if let Some(snap) = c.snapshots.get(step.snapshot) {
                        for (name, board) in [("us", &snap.board_a), ("them", &snap.board_b)] {
                            let units: Vec<String> = board
                                .iter()
                                .map(|u| format!("{} {}/{}", u.name, u.attack, u.health))
                                .collect();
                            let _ = writeln!(out, "      {name:>4}: {}", units.join(", "));
                        }
                    }
                }
            }
        }
        total += 1;
        let _ = writeln!(
            out,
            "\n=== turn {} · {}",
            source.info(i).turn,
            source.title(i)
        );
        let Some(engine) = real.alt.as_deref() else {
            let _ = writeln!(
                out,
                "  no engine version: {}",
                real.warning.as_deref().unwrap_or("?")
            );
            continue;
        };
        let strikes =
            |c: &CombatPlayback| c.steps.iter().filter(|s| s.phase == Phase::Strike).count();
        let _ = writeln!(
            out,
            "  real:   {} ({} strikes)",
            real.result,
            strikes(&real)
        );
        let _ = writeln!(
            out,
            "  engine: {} ({} strikes)",
            engine.result,
            strikes(engine)
        );
        if real.result == engine.result {
            same_result += 1;
        }
        if let Some(w) = &engine.warning {
            for part in w.split(" · ") {
                if !part.starts_with('⚠') {
                    let _ = writeln!(out, "  note: {part}");
                }
            }
        }
        // After the last strike, the result says how the combat ended (in Duos the log can show a
        // warband stepping in after its team has won).
        let first = (1..strikes(&real).min(strikes(engine)))
            .find(|&k| boards_after(&real, k) != boards_after(engine, k));
        let Some(k) = first else {
            if strikes(&real) == strikes(engine) {
                same += 1;
                let _ = writeln!(out, "  boards match after every strike");
            } else {
                let _ = writeln!(out, "  boards match while both combats last");
            }
            continue;
        };
        let _ = writeln!(out, "  boards differ after strike {k}:");
        let (r, e) = (boards_after(&real, k), boards_after(engine, k));
        for (side, name) in [(0, "us"), (1, "them")] {
            let rb = r.as_ref().map(|b| b[side].clone()).unwrap_or_default();
            let eb = e.as_ref().map(|b| b[side].clone()).unwrap_or_default();
            let only = |a: &Board, b: &Board| {
                let mut rest = b.clone();
                a.iter()
                    .filter(|x| match rest.iter().position(|y| y == *x) {
                        Some(i) => {
                            rest.remove(i);
                            false
                        }
                        None => true,
                    })
                    .map(|(n, at, h)| format!("{n} {at}/{h}"))
                    .collect::<Vec<_>>()
            };
            let (or, oe) = (only(&rb, &eb), only(&eb, &rb));
            if !or.is_empty() || !oe.is_empty() {
                let _ = writeln!(
                    out,
                    "    {name}: real has [{}], engine has [{}]",
                    or.join(", "),
                    oe.join(", ")
                );
            }
        }
        for (version, c) in [("real", &real), ("engine", engine)] {
            let texts = strike_texts(c, k);
            if texts.is_empty() {
                let _ = writeln!(out, "    {version} strike {k}: (none)");
            }
            for (j, t) in texts.iter().enumerate() {
                let head = if j == 0 {
                    format!("{version} strike {k}:")
                } else {
                    String::new()
                };
                let _ = writeln!(out, "    {head:<16} {t}");
            }
        }
    }
    let _ = writeln!(
        out,
        "\n{total} combats: {same_result} with the real result, {same} with the same boards after every strike"
    );
    Ok(out)
}

impl Source for PowerLogSource {
    fn len(&self) -> usize {
        self.frames.len()
    }

    fn info(&self, frame: usize) -> FrameInfo {
        self.frames[frame].info
    }

    fn title(&self, frame: usize) -> String {
        self.frames[frame].title.clone()
    }

    fn players(&self) -> &[String] {
        &self.names
    }

    fn tavern(&self, frame: usize, _player: usize) -> TavernView {
        self.frames[frame].tavern.clone()
    }

    fn combat(&self, frame: usize) -> Option<CombatPlayback> {
        self.frames[frame].combat.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::App;
    use crate::tui::screen;
    use seaglass_replay::parse_power_log_str;
    use seaglass_tools::render::Glyphs;

    /// A minimal Duos game: turn 1's Tavern (Abyssal Envoy in the shop), then a combat of the
    /// local player's 4/5 against player 7's buffed Abyssal Envoy: one attack, both die (a draw).
    const LOG: &str = "CREATE_GAME
    GameEntity EntityID=1
        tag=ZONE value=PLAY
    Player EntityID=2 PlayerID=5 GameAccountId=[hi=1 lo=2]
        tag=BACON_DUO_TEAMMATE_PLAYER_ID value=6
    Player EntityID=3 PlayerID=10 GameAccountId=[hi=0 lo=0]
TAG_CHANGE Entity=GameEntity tag=TURN value=1 
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
FULL_ENTITY - Creating ID=22 CardID=BG36_311
    tag=CONTROLLER value=10
    tag=CARDTYPE value=MINION
    tag=ZONE value=PLAY
    tag=ZONE_POSITION value=1
    tag=ATK value=3
    tag=HEALTH value=4
    tag=TECH_LEVEL value=3
TAG_CHANGE Entity=GameEntity tag=STEP value=MAIN_ACTION 
TAG_CHANGE Entity=GameEntity tag=STEP value=MAIN_END 
TAG_CHANGE Entity=22 tag=ZONE value=REMOVEDFROMGAME 
TAG_CHANGE Entity=GameEntity tag=TURN value=2 
TAG_CHANGE Entity=GameEntity tag=BACON_IN_COMBAT_PHASE value=1 
    FULL_ENTITY - Creating ID=30 CardID=BG20_HERO_100
        tag=CONTROLLER value=5
        tag=CARDTYPE value=HERO
        tag=PLAYER_ID value=7
        tag=PLAYER_TECH_LEVEL value=4
        tag=HEALTH value=30
        tag=ZONE value=SETASIDE
    TAG_CHANGE Entity=30 tag=BACON_COMBAT_PHASE_HERO value=1 
    TAG_CHANGE Entity=GameEntity tag=1453 value=1 
    FULL_ENTITY - Creating ID=32 CardID=BG36_311
        tag=CONTROLLER value=5
        tag=CARDTYPE value=MINION
        tag=ZONE value=SETASIDE
        tag=ATK value=6
        tag=HEALTH value=4
        tag=TECH_LEVEL value=3
    TAG_CHANGE Entity=GameEntity tag=1453 value=0 
    TAG_CHANGE Entity=GameEntity tag=1453 value=1 
    TAG_CHANGE Entity=Me#1234 tag=BACON_CURRENT_COMBAT_PLAYER_ID value=5 
    TAG_CHANGE Entity=Opponent tag=BACON_CURRENT_COMBAT_PLAYER_ID value=7 
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

    fn source() -> PowerLogSource {
        let log: String = LOG
            .lines()
            .map(|l| format!("D 00:00:00.0000000 GameState.DebugPrintPower() - {l}\n"))
            .collect();
        let games = parse_power_log_str(&log).unwrap();
        assert_eq!(games.len(), 1);
        PowerLogSource::new(&games[0])
    }

    #[test]
    fn a_log_game_plays_its_tavern_and_its_combat_strike_by_strike() {
        let source = source();
        let kinds: Vec<Kind> = (0..source.len()).map(|i| source.info(i).kind).collect();
        assert_eq!(kinds, [Kind::TurnStart, Kind::Combat, Kind::End]);
        let start = source.tavern(0, 0);
        assert_eq!(start.rows[0].units[0].name, "Abyssal Envoy", "the shop");
        assert_eq!(start.rows[1].units.len(), 1, "the board");

        let combat = source.combat(1).expect("the combat");
        assert_eq!(combat.version, "real");
        assert_eq!(combat.result, "Draw");
        let phases: Vec<Phase> = combat.steps.iter().map(|s| s.phase).collect();
        assert_eq!(
            phases,
            [Phase::Start, Phase::Strike, Phase::Settle, Phase::End]
        );
        let strike = &combat.steps[1];
        assert_eq!(strike.attacker, Some((Side::A, 21)));
        assert_eq!(strike.target, Some((Side::B, 40)));
        assert_eq!(strike.dying.len(), 2, "both minions die");
        assert!(strike.labels[0].contains("(Player)"), "{:?}", strike.labels);
        assert!(
            !strike.labels[1].starts_with("BG20_HERO"),
            "{:?}",
            strike.labels
        );
        assert!(strike.labels[1].contains("Tier 4"), "{:?}", strike.labels);

        // The engine's version follows the real attack and agrees.
        let engine = combat.alt.as_ref().expect("the engine's version");
        assert_eq!(engine.version, "engine");
        assert_eq!(engine.result, "Draw");
        assert!(combat.warning.is_none(), "{:?}", combat.warning);

        // Every frame and step draws, both versions, at any size.
        let mut app = App::new(Box::new(source), "Power.log".into(), 0, Glyphs::UNICODE);
        loop {
            for (w, h) in [(172, 50), (100, 30), (40, 12)] {
                screen::draw(&app, w, h);
            }
            if app.combat.is_some() {
                app.toggle_alt();
                screen::draw(&app, 120, 40);
                app.toggle_alt();
            }
            if !app.next() {
                break;
            }
        }
    }
}
