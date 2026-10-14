//! `replay_cli validate`: runs the engine on every combat of a log and reports where it disagrees
//! with the real results.

use std::collections::{BTreeMap, BTreeSet};

use seaglass::model::{BattleOutcome, DeityKind};
use seaglass_replay::cards::{engine_card, CardKind, MatchedBy};
use seaglass_replay::reconstruct::{CombatSetup, PlayerSetup, ServerEvent};
use seaglass_replay::validate::{deity_kind_of, engine_input, validate, EngineInput, Validation};
use seaglass_replay::GameReplay;

/// `replay_cli cards`: every minion and spell in the log, and how it maps to the engine.
pub fn print_cards(games: &[GameReplay]) {
    let mut all: BTreeMap<String, (String, String, bool, BTreeSet<usize>)> = BTreeMap::new();
    for g in games {
        for (cid, (name, ctype, golden)) in &g.cards_seen {
            let e = all.entry(cid.clone()).or_insert((
                name.clone(),
                ctype.clone(),
                *golden,
                BTreeSet::new(),
            ));
            e.3.insert(g.game_index);
        }
    }
    let mut missing_minions = Vec::new();
    let mut missing_spells = Vec::new();
    let mut by_name = Vec::new();
    let mut mapped = 0;
    for (cid, (name, ctype, _golden, games)) in &all {
        let games = games
            .iter()
            .map(|g| g.to_string())
            .collect::<Vec<_>>()
            .join(",");
        match engine_card(cid, name) {
            Some(c) => {
                mapped += 1;
                if c.matched_by == MatchedBy::Name || c.matched_by == MatchedBy::Variant {
                    by_name.push(format!("{name} ({cid}) -> {} [{:?}]", c.name, c.matched_by));
                }
                if ctype == "MINION" && c.kind == CardKind::Spell {
                    missing_minions.push(format!("{name} ({cid}) [games {games}]"));
                }
            }
            None if ctype == "MINION" => {
                missing_minions.push(format!("{name} ({cid}) [games {games}]"))
            }
            None => missing_spells.push(format!("{name} ({cid}) [games {games}]")),
        }
    }
    println!(
        "{} cards in the log, {mapped} mapped to engine cards",
        all.len()
    );
    println!("\nMapped by name or variant ({}):", by_name.len());
    for l in &by_name {
        println!("  {l}");
    }
    println!(
        "\nMinions the engine doesn't implement ({}):",
        missing_minions.len()
    );
    for l in &missing_minions {
        println!("  {l}");
    }
    println!(
        "\nSpells the engine doesn't implement (incl. hero-power and trinket cards) ({}):",
        missing_spells.len()
    );
    for l in &missing_spells {
        println!("  {l}");
    }
}

pub struct Options {
    pub runs: u32,
    /// Runs of the guided replay (the engine attacking the real targets).
    pub guided_runs: u32,
    pub seed: u64,
    pub verbose: bool,
    pub turn: Option<u32>,
    /// Games to validate (1-indexed); empty: all.
    pub games: Vec<usize>,
}

fn outcome_str(outcome: BattleOutcome, damage: i32) -> String {
    match outcome {
        BattleOutcome::AWin => format!("WIN {damage}"),
        BattleOutcome::BWin => format!("LOSS {damage}"),
        BattleOutcome::Draw => "TIE".to_string(),
    }
}

fn pct(n: u32, total: u32) -> f64 {
    100.0 * n as f64 / total.max(1) as f64
}

fn player_line(p: &PlayerSetup) -> String {
    let tag = |k: &str| {
        p.player_tags
            .get(k)
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
    };
    let mut extras = Vec::new();
    if let Some((a, h)) = p.deity {
        extras.push(format!("Deity {a}/{h}"));
    }
    if p.tags_known {
        let (sa, sh) = (
            tag("TAVERN_SPELL_ATTACK_INCREASE"),
            tag("TAVERN_SPELL_HEALTH_INCREASE"),
        );
        if (sa, sh) != (0, 0) {
            extras.push(format!("spells +{sa}/+{sh}"));
        }
        let (ba, bh) = (tag("3874"), tag("3924"));
        if (ba, bh) != (0, 0) {
            extras.push(format!("Beetles +{ba}/+{bh}"));
        }
    } else {
        extras.push("player tags unknown".to_string());
    }
    for e in &p.player_enchantments {
        if matches!(
            e.card_id.as_str(),
            "Bacon_TagTransferPlayerE"
                | "TB_BaconShop_3ofKindChecke"
                | "TB_BaconShop_8P_PlayerE"
                | "TB_BaconShop_FXWatcher"
                | "Bacon_Free_Refresh_Player_Ench"
        ) {
            continue;
        }
        extras.push(format!(
            "{} [{}/{}]",
            e.name.trim_end_matches(" [DNT]"),
            e.script_data[0],
            e.script_data[1]
        ));
    }
    if let Some(hp) = &p.hero_power {
        extras.push(format!(
            "HP {hp}{}",
            if p.hero_power_start_of_combat {
                " (SoC)"
            } else {
                ""
            }
        ));
    }
    if !p.trinkets.is_empty() {
        extras.push(format!("trinkets {}", p.trinkets.join(" ")));
    }
    format!(
        "{} (player {}) T{} {}+{}hp{} | {}",
        p.hero_name,
        p.player_id,
        p.tier,
        p.hero_health,
        p.hero_armor,
        if p.fought { "" } else { " (did not fight)" },
        extras.join(", ")
    )
}

fn print_team(label: &str, players: &[PlayerSetup]) {
    for (i, p) in players.iter().enumerate() {
        let first = if i == 0 { "first" } else { "second" };
        println!("  {label} {first}: {}", player_line(p));
        println!(
            "      board: {}",
            if p.board.is_empty() {
                "(empty)".to_string()
            } else {
                p.board
                    .iter()
                    .map(|m| {
                        if m.dark_gifts.is_empty() {
                            m.short()
                        } else {
                            format!("{} {{{}}}", m.short(), m.dark_gifts.join(", "))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        );
        if !p.hand.is_empty() {
            println!(
                "      hand:  {}",
                p.hand
                    .iter()
                    .map(|c| match &c.minion {
                        Some(m) => m.short(),
                        None => c.name.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
}

fn print_events(setup: &CombatSetup) {
    println!("  server log:");
    let mut n = 0;
    for e in &setup.events {
        match e {
            ServerEvent::Entry { side, player_id } => {
                println!("      -- {side:?}: player {player_id} enters");
            }
            ServerEvent::Attack {
                side,
                attacker,
                defender,
            } => {
                n += 1;
                println!(
                    "      {n:>3}. {side:?} {} -> {}",
                    attacker.short(),
                    defender.short()
                );
            }
            ServerEvent::Trigger {
                side,
                source,
                depth,
                ..
            } => {
                println!(
                    "           {}trigger {side:?} {source}",
                    "  ".repeat(*depth)
                );
            }
            ServerEvent::Summon { side, unit } => {
                println!("           + {side:?} {}", unit.short());
            }
            ServerEvent::Death { side, unit } => {
                println!("           x {side:?} {}", unit.short());
            }
        }
    }
}

fn print_validation(setup: &CombatSetup, input: &EngineInput, v: &Validation, verbose: bool) {
    println!(
        "  real: {}{}",
        outcome_str(setup.result.outcome, setup.result.damage),
        setup
            .damage_cap
            .map(|c| format!(" (damage cap {c})"))
            .unwrap_or_default()
    );
    if let Some(winner) = setup.result.winner_player_id {
        let players = setup.team_a.players.iter().chain(&setup.team_b.players);
        let tier = players
            .clone()
            .find(|p| p.player_id == winner)
            .map_or(0, |p| p.tier);
        let tiers: u32 = setup.result.survivors.iter().map(|m| m.tier).sum();
        println!(
            "  real damage: winner player {winner} (Tier {tier}) with {} survivor(s) [{}], Tier + their tiers = {}",
            setup.result.survivors.len(),
            setup
                .result
                .survivors
                .iter()
                .map(|m| format!("{} T{}", m.name, m.tier))
                .collect::<Vec<_>>()
                .join(", "),
            tier + tiers
        );
    }
    println!(
        "  engine ({} runs): A wins {:.1}% | draws {:.1}% | B wins {:.1}%   real outcome {:.1}%, real outcome+damage {:.1}%{}",
        v.runs,
        pct(v.a_wins, v.runs),
        pct(v.draws, v.runs),
        pct(v.b_wins, v.runs),
        pct(v.real_outcome_runs, v.runs),
        pct(v.real_exact_runs, v.runs),
        if v.deterministic { "  [deterministic]" } else { "" }
    );
    let mut top: Vec<(&i32, &u32)> = v.damage.iter().collect();
    top.sort_by(|a, b| b.1.cmp(a.1));
    println!(
        "  engine damage (signed, A's view): {}",
        top.iter()
            .take(8)
            .map(|(d, n)| format!("{d:+}: {:.1}%", pct(**n, v.runs)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    if let Some(d) = &v.divergence {
        println!(
            "  best run (seed {}): matches {}/{} steps ({} with stats), ends {}",
            d.seed,
            d.matched,
            d.real_len,
            d.matched_with_stats,
            outcome_str(d.outcome, d.damage.abs())
        );
        if let Some((i, real, eng)) = &d.first_stat_diff {
            println!("    first stats difference at step {}:", i + 1);
            println!("      real:   {}", real.describe());
            println!("      engine: {}", eng.describe());
        }
        if let Some((i, real, eng)) = &d.first_diff {
            println!("    first divergence at step {} ({}):", i + 1, d.kind);
            println!(
                "      real:   {}",
                real.as_ref()
                    .map_or("(combat over)".to_string(), |s| s.describe())
            );
            println!(
                "      engine: {}",
                eng.as_ref()
                    .map_or("(combat over)".to_string(), |s| s.describe())
            );
        }
    }
    if let Some(g) = &v.guided {
        println!(
            "  guided replay (real targets, {} runs): A wins {:.1}% | draws {:.1}% | B wins {:.1}%   real outcome {:.1}%, real outcome+damage {:.1}%",
            g.runs,
            pct(g.a_wins, g.runs),
            pct(g.draws, g.runs),
            pct(g.b_wins, g.runs),
            pct(g.real_outcome_runs, g.runs),
            pct(g.real_exact_runs, g.runs),
        );
        if let Some(d) = &g.divergence {
            println!(
                "    best guided run (seed {}): matches {}/{} steps ({} with stats), ends {}",
                d.seed,
                d.matched,
                d.real_len,
                d.matched_with_stats,
                outcome_str(d.outcome, d.damage.abs())
            );
            if let Some((i, real, eng)) = &d.first_stat_diff {
                println!("    first stats difference at step {}:", i + 1);
                println!("      real:   {}", real.describe());
                println!("      engine: {}", eng.describe());
            }
            if let Some((i, real, eng)) = &d.first_diff {
                println!("    first divergence at step {} ({}):", i + 1, d.kind);
                println!(
                    "      real:   {}",
                    real.as_ref()
                        .map_or("(combat over)".to_string(), |s| s.describe())
                );
                println!(
                    "      engine: {}",
                    eng.as_ref()
                        .map_or("(combat over)".to_string(), |s| s.describe())
                );
            }
            if let Some((k, why)) = &g.lost {
                println!(
                    "    stopped following the real targets at attack {}: {why}",
                    k + 1
                );
            }
        }
    }
    if !input.unimplemented.is_empty() {
        println!(
            "  not implemented (vanilla bodies): {}",
            input
                .unimplemented
                .iter()
                .map(|c| format!("{} ({})", c.name, c.card_id))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if verbose && !input.matched_by_name.is_empty() {
        println!(
            "  matched by name: {}",
            input
                .matched_by_name
                .iter()
                .map(|c| format!("{} ({})", c.name, c.card_id))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    for g in &input.gaps {
        println!("  gap: {g}");
    }
    for n in &setup.notes {
        println!("  note: {n}");
    }
    for f in &v.flags {
        println!("  FLAG: {f}");
    }
}

struct Flagged {
    game: usize,
    turn: u32,
    flags: Vec<String>,
    cards: BTreeSet<String>,
}

pub fn run(games: &[GameReplay], opts: &Options) {
    let mut total = 0;
    let mut flagged: Vec<Flagged> = Vec::new();
    let mut unimplemented: BTreeMap<String, BTreeSet<(usize, u32)>> = BTreeMap::new();
    for g in games {
        if !opts.games.is_empty() && !opts.games.contains(&g.game_index) {
            continue;
        }
        let deity = g
            .combats
            .first()
            .and_then(|c| deity_kind_of(&c.deity_dbid))
            .unwrap_or(DeityKind::CThun);
        for setup in &g.combats {
            if opts.turn.is_some_and(|t| t != setup.turn) {
                continue;
            }
            total += 1;
            let input = engine_input(setup, deity);
            let v = validate(setup, &input, opts.runs, opts.guided_runs, opts.seed);
            for c in &input.unimplemented {
                unimplemented
                    .entry(format!("{} ({})", c.name, c.card_id))
                    .or_default()
                    .insert((g.game_index, setup.turn));
            }
            let is_flagged = !v.flags.is_empty();
            println!(
                "\n=== Game {} ({}) — turn {} — {}{}",
                g.game_index,
                if setup.is_duos { "Duos" } else { "Solo" },
                setup.turn,
                outcome_str(setup.result.outcome, setup.result.damage),
                if is_flagged { "  [FLAGGED]" } else { "" }
            );
            print_team("A", &setup.team_a.players);
            print_team("B", &setup.team_b.players);
            print_validation(setup, &input, &v, opts.verbose);
            if opts.verbose {
                print_events(setup);
            }
            if is_flagged {
                let cards = setup
                    .team_a
                    .players
                    .iter()
                    .chain(&setup.team_b.players)
                    .flat_map(|p| p.board.iter().map(|m| m.name.clone()))
                    .collect();
                flagged.push(Flagged {
                    game: g.game_index,
                    turn: setup.turn,
                    flags: v.flags.clone(),
                    cards,
                });
            }
        }
    }
    println!("\n================================================================================");
    println!(
        " SUMMARY: {} combat(s), {} matched, {} flagged ({} runs each)",
        total,
        total - flagged.len(),
        flagged.len(),
        opts.runs
    );
    for f in &flagged {
        println!("  game {} turn {}: {}", f.game, f.turn, f.flags.join("; "));
    }
    let mut card_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for f in &flagged {
        for c in &f.cards {
            *card_counts.entry(c.as_str()).or_default() += 1;
        }
    }
    if !card_counts.is_empty() {
        let mut cards: Vec<(&&str, &usize)> = card_counts.iter().collect();
        cards.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        println!(
            "  cards in flagged combats (count): {}",
            cards
                .iter()
                .map(|(c, n)| format!("{c} ({n})"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !unimplemented.is_empty() {
        println!("  minions the engine doesn't implement (game/turn):");
        for (card, where_) in &unimplemented {
            println!(
                "    {card}: {}",
                where_
                    .iter()
                    .map(|(g, t)| format!("g{g}t{t}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
    }
}
