//! Text rendering of re-simulated replays ([`Timeline`], `engine/docs/replays.md`).

use seaglass::replay::{pb, Frame, FrameKind, Timeline};
use seaglass::{BattleOutcome, CardId};

use crate::fmt::{
    describe_action, format_deity_kind, format_unit_list, print_combat_trace,
    print_player_snapshot, print_step_diff, PoolCounts,
};

/// A player's display name: their setup name, or `P1`, `P2`, ...
pub fn player_name(timeline: &Timeline, p: usize) -> String {
    timeline
        .header
        .players
        .get(p)
        .map(|setup| setup.name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("P{}", p + 1))
}

/// The copies left in the pool player `p` draws from, at one frame.
pub struct FramePool<'a> {
    card_ids: &'a [CardId],
    remaining: &'a [u32],
}

impl PoolCounts for FramePool<'_> {
    fn copies_left(&self, card_id: CardId) -> u32 {
        self.card_ids
            .iter()
            .position(|&id| id == card_id)
            .map_or(0, |i| self.remaining[i])
    }
}

/// The players of unit `u`: `[u]` in Solo, team `u`'s two players in Duos.
pub fn unit_players(timeline: &Timeline, u: usize) -> Vec<usize> {
    if timeline.header.duos {
        vec![2 * u, 2 * u + 1]
    } else {
        vec![u]
    }
}

/// A unit's display name: the player's (Solo), or `team 1 (A + B)` (Duos).
pub fn unit_name(timeline: &Timeline, u: usize) -> String {
    if timeline.header.duos {
        format!(
            "team {} ({} + {})",
            u + 1,
            player_name(timeline, 2 * u),
            player_name(timeline, 2 * u + 1)
        )
    } else {
        player_name(timeline, u)
    }
}

pub fn frame_pool<'a>(timeline: &'a Timeline, frame: &'a Frame, p: usize) -> FramePool<'a> {
    // One pool per unit (a player, a Duos team), or one shared by everyone.
    let slot = if frame.pool_remaining.len() == 1 {
        0
    } else if timeline.header.duos {
        p / 2
    } else {
        p
    };
    FramePool {
        card_ids: &timeline.card_ids,
        remaining: &frame.pool_remaining[slot],
    }
}

/// What an actor noted about an action, e.g. `  [value 0.512, p 0.31, forced]`.
pub fn note_text(note: Option<&pb::Note>) -> String {
    let Some(note) = note else {
        return String::new();
    };
    let mut parts = Vec::new();
    if let Some(value) = note.value {
        parts.push(format!("value {value:.3}"));
    }
    if let Some(logp) = note.logp {
        parts.push(format!("p {:.2}", logp.exp()));
    }
    if note.forced {
        parts.push("forced".to_string());
    }
    for (key, value) in &note.extra {
        parts.push(format!("{key} {value:.3}"));
    }
    if !note.candidates.is_empty() {
        parts.push(format!("{} candidates", note.candidates.len()));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("  [{}]", parts.join(", "))
    }
}

/// One line describing frame `i`: the call that produced it.
pub fn frame_title(timeline: &Timeline, i: usize) -> String {
    let name = |p: usize| player_name(timeline, p);
    let frame = &timeline.frames[i];
    match &frame.kind {
        FrameKind::Start => "match start".to_string(),
        FrameKind::StartTurn { player, turn } => {
            format!("{} starts turn {turn}", unit_name(timeline, *player))
        }
        FrameKind::Act {
            player,
            action,
            note,
            ..
        } => {
            let before = &timeline.frames[i - 1].players[*player];
            format!(
                "{}: {}{}",
                name(*player),
                describe_action(before, action),
                note_text(note.as_ref())
            )
        }
        FrameKind::Combat { a, b, result, .. } => {
            let outcome = match result.outcome {
                BattleOutcome::AWin => format!("{} wins, {} damage", name(*a), result.hero_damage),
                BattleOutcome::BWin => format!("{} wins, {} damage", name(*b), result.hero_damage),
                BattleOutcome::Draw => "draw".to_string(),
            };
            format!("combat {} vs {}: {outcome}", name(*a), name(*b))
        }
        FrameKind::DuosCombat { a, b, result, .. } => {
            let unit = |u: usize| unit_name(timeline, u);
            let outcome = match result.outcome {
                BattleOutcome::AWin => format!("{} wins, {} damage", unit(*a), result.hero_damage),
                BattleOutcome::BWin => format!("{} wins, {} damage", unit(*b), result.hero_damage),
                BattleOutcome::Draw => "draw".to_string(),
            };
            format!("combat {} vs {}: {outcome}", unit(*a), unit(*b))
        }
        FrameKind::GhostCombat {
            unit,
            ghost,
            result,
            ..
        } => {
            let outcome = match result.outcome() {
                BattleOutcome::AWin => "wins".to_string(),
                BattleOutcome::BWin => format!("loses, {} damage", result.hero_damage()),
                BattleOutcome::Draw => "draw".to_string(),
            };
            format!(
                "combat {} vs the ghost of {}: {outcome}",
                unit_name(timeline, *unit),
                unit_name(timeline, *ghost)
            )
        }
        FrameKind::SetPairs { pairs } => {
            let pairs: Vec<String> = pairs
                .iter()
                .map(|&(a, b)| format!("{} vs {}", name(a), name(b)))
                .collect();
            format!("lobby pairings: {}", pairs.join(", "))
        }
        FrameKind::DeliverGifts => "lobby gifts delivered".to_string(),
        FrameKind::RecordDeaths { alive } => {
            let out: Vec<String> = (0..alive.len()).filter(|&p| !alive[p]).map(name).collect();
            if out.is_empty() {
                "lobby: everyone is still alive".to_string()
            } else {
                format!("lobby: out of the game: {}", out.join(", "))
            }
        }
        FrameKind::End { winner, placements } => {
            let result = winner.map_or("draw".to_string(), |w| {
                format!("{} wins", unit_name(timeline, w))
            });
            if placements.is_empty() {
                let health: Vec<String> = frame
                    .players
                    .iter()
                    .enumerate()
                    .map(|(p, s)| format!("{} {}", name(p), s.health))
                    .collect();
                format!("game over: {result} ({})", health.join(", "))
            } else {
                let mut order: Vec<usize> = (0..placements.len()).collect();
                order.sort_by_key(|&u| placements[u]);
                let places: Vec<String> = order
                    .iter()
                    .map(|&u| format!("{}. {}", placements[u], unit_name(timeline, u)))
                    .collect();
                format!("game over: {result} ({})", places.join(", "))
            }
        }
    }
}

/// Print frame `i`: its title and what its call changed (the acting player's state changes, or
/// the combat with its full event log if `combat_log`).
pub fn print_frame(timeline: &Timeline, i: usize, combat_log: bool) {
    let frame = &timeline.frames[i];
    println!("[{i}] turn {} · {}", frame.turn, frame_title(timeline, i));
    let Some(prev) = i.checked_sub(1).map(|j| &timeline.frames[j]) else {
        return;
    };
    match &frame.kind {
        FrameKind::Start => {}
        FrameKind::StartTurn { player, .. } => {
            for p in unit_players(timeline, *player) {
                print_player_snapshot(
                    &player_name(timeline, p),
                    &frame.players[p],
                    &frame_pool(timeline, frame, p),
                );
            }
        }
        FrameKind::Act { player, .. } => {
            let p = *player;
            print_step_diff(
                &prev.players[p],
                &frame.players[p],
                &frame_pool(timeline, frame, p),
            );
        }
        FrameKind::Combat { a, b, result, .. } => {
            let (a, b) = (*a, *b);
            let (name_a, name_b) = (player_name(timeline, a), player_name(timeline, b));
            if combat_log {
                print_combat_trace(
                    frame.turn,
                    [&name_a, &name_b],
                    &prev.players[a],
                    &prev.players[b],
                    result,
                    &frame.players[a],
                    &frame.players[b],
                    &frame_pool(timeline, frame, a),
                );
            } else {
                for (p, name, survivors) in [
                    (a, &name_a, &result.survivors_a),
                    (b, &name_b, &result.survivors_b),
                ] {
                    let (before, after) = (&prev.players[p], &frame.players[p]);
                    println!(
                        "  {name}: HP {}+{} -> {}+{}, {} survivor(s)",
                        before.health,
                        before.armor,
                        after.health,
                        after.armor,
                        survivors.len()
                    );
                    print!("{}", format_unit_list(survivors, "    ", None));
                }
                println!("  ({} combat events; `c` shows them)", result.events.len());
            }
        }
        FrameKind::DuosCombat { a, b, .. }
        | FrameKind::GhostCombat {
            unit: a, ghost: b, ..
        } => {
            let ghost = matches!(frame.kind, FrameKind::GhostCombat { .. });
            let units = if ghost { vec![*a] } else { vec![*a, *b] };
            for u in units {
                for p in unit_players(timeline, u) {
                    let (before, after) = (&prev.players[p], &frame.players[p]);
                    println!(
                        "  {}: HP {}+{} -> {}+{}, board {} -> {}",
                        player_name(timeline, p),
                        before.health,
                        before.armor,
                        after.health,
                        after.armor,
                        before.board.len(),
                        after.board.len()
                    );
                }
            }
        }
        FrameKind::DeliverGifts => {
            for p in 0..frame.players.len() {
                if prev.players[p] != frame.players[p] {
                    println!("  {}:", player_name(timeline, p));
                    print_step_diff(
                        &prev.players[p],
                        &frame.players[p],
                        &frame_pool(timeline, frame, p),
                    );
                }
            }
        }
        FrameKind::SetPairs { .. } | FrameKind::RecordDeaths { .. } | FrameKind::End { .. } => {}
    }
}

/// Print every player's full state at frame `i`.
pub fn print_state(timeline: &Timeline, i: usize) {
    let frame = &timeline.frames[i];
    println!("[{i}] turn {} · {}", frame.turn, frame_title(timeline, i));
    for (p, state) in frame.players.iter().enumerate() {
        print_player_snapshot(
            &player_name(timeline, p),
            state,
            &frame_pool(timeline, frame, p),
        );
    }
    let rngs: Vec<String> = frame
        .rng_states
        .iter()
        .map(|s| format!("{s:#018x}"))
        .collect();
    println!("  RNG state: {}", rngs.join(", "));
}

/// Print the replay's setup, every combat and the result.
pub fn print_summary(timeline: &Timeline) {
    let header = &timeline.header;
    let pool = match header.pool() {
        pb::PoolMode::Shared => format!("shared pool, seed {}", header.shared_seed),
        _ => "per-player pools".to_string(),
    };
    println!(
        "seaglass {} · format {} · catalog {:?} · {pool}{}",
        header.engine,
        header.format,
        header.catalog,
        if header.duos { " · Duos" } else { "" }
    );
    for (p, setup) in header.players.iter().enumerate() {
        let seed = if header.pool() == pb::PoolMode::Shared {
            String::new()
        } else {
            format!(", seed {}", setup.seed)
        };
        let armor = if setup.hero.is_none() && setup.hero_options.is_empty() {
            format!(", {} armor", setup.armor)
        } else {
            String::new()
        };
        let heroes = if setup.hero_options.is_empty() {
            String::new()
        } else {
            format!(", offered heroes {:?}", setup.hero_options)
        };
        println!(
            "  {}: {}{armor}, shop spells {}{heroes}{seed}",
            player_name(timeline, p),
            format_deity_kind(seaglass::replay::deity_from_pb(setup.deity())),
            if setup.shop_spells { "on" } else { "off" },
        );
    }
    for (key, value) in &header.meta {
        println!("  {key} = {value}");
    }
    let actions = timeline.decisions().count();
    println!(
        "{} frames, {actions} actions, {} turns",
        timeline.len(),
        timeline.last_turn()
    );
    for (i, frame) in timeline.frames.iter().enumerate() {
        let units = match &frame.kind {
            FrameKind::Combat { a, b, .. } | FrameKind::DuosCombat { a, b, .. } => vec![*a, *b],
            FrameKind::GhostCombat { unit, .. } => vec![*unit],
            _ => Vec::new(),
        };
        if !units.is_empty() {
            let health: Vec<String> = units
                .iter()
                .map(|&u| {
                    let p = unit_players(timeline, u)[0];
                    format!(
                        "{} {}+{}",
                        unit_name(timeline, u),
                        frame.players[p].health,
                        frame.players[p].armor
                    )
                })
                .collect();
            println!(
                "  [{i}] turn {}: {}  → {}",
                frame.turn,
                frame_title(timeline, i),
                health.join(", ")
            );
        }
    }
    match timeline.frames.last().map(|f| &f.kind) {
        Some(FrameKind::End { .. }) => {
            println!("{}", frame_title(timeline, timeline.len() - 1));
        }
        _ => println!("the replay ends before the match does"),
    }
    print_problems(timeline);
}

/// Print why a replay does not re-simulate faithfully, if it does not. Returns whether it does.
pub fn print_problems(timeline: &Timeline) -> bool {
    if let Some(engine) = timeline.engine_mismatch() {
        println!(
            "note: recorded by seaglass {engine}, re-simulated by {}",
            seaglass::replay::engine_version()
        );
    }
    for d in &timeline.divergences {
        println!(
            "DIVERGED at record {} (frame {}): {}",
            d.record, d.frame, d.message
        );
    }
    if let Some(e) = &timeline.error {
        println!("STOPPED at record {}: {}", e.record, e.message);
    }
    timeline.is_faithful()
}
