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

pub fn frame_pool<'a>(timeline: &'a Timeline, frame: &'a Frame, p: usize) -> FramePool<'a> {
    // One pool per player, or one shared by everyone.
    let slot = if frame.pool_remaining.len() == 1 {
        0
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
        FrameKind::StartTurn { player, turn } => format!("{} starts turn {turn}", name(*player)),
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
        FrameKind::End { winner } => {
            let health: Vec<String> = frame
                .players
                .iter()
                .enumerate()
                .map(|(p, s)| format!("{} {}", name(p), s.health))
                .collect();
            let result = winner.map_or("draw".to_string(), |w| format!("{} wins", name(w)));
            format!("game over: {result} ({})", health.join(", "))
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
            let p = *player;
            print_player_snapshot(
                &player_name(timeline, p),
                &frame.players[p],
                &frame_pool(timeline, frame, p),
            );
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
        "seaglass {} · format {} · catalog {:?} · {pool}",
        header.engine, header.format, header.catalog
    );
    for (p, setup) in header.players.iter().enumerate() {
        let seed = if header.pool() == pb::PoolMode::Shared {
            String::new()
        } else {
            format!(", seed {}", setup.seed)
        };
        println!(
            "  {}: {}, {} armor, shop spells {}{seed}",
            player_name(timeline, p),
            format_deity_kind(seaglass::replay::deity_from_pb(setup.deity())),
            setup.armor,
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
        if let FrameKind::Combat { a, b, .. } = &frame.kind {
            let health: Vec<String> = [*a, *b]
                .iter()
                .map(|&p| {
                    format!(
                        "{} {}+{}",
                        player_name(timeline, p),
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
