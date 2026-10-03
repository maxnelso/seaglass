//! CLI inspector and exporter for Hearthstone Battlegrounds `Power.log` replays.
//!
//! Usage:
//!   cargo run --bin replay_cli                                      # List games + summary of Game 1 in ~/scratch/Power.log
//!   cargo run --bin replay_cli -- list [/path/to/Power.log]         # List all games in Power.log
//!   cargo run --bin replay_cli -- summary [/path/to/Power.log] [--game 1]
//!   cargo run --bin replay_cli -- turn <TURN> [/path/to/Power.log] [--game 1]
//!   cargo run --bin replay_cli -- export-yaml [/path/to/Power.log] [--game 1] [--turn <T>] [-o out.yaml]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

use seaglass::{simulate_batch, BattleOutcome, GameState};
use seaglass_replay::{
    parse_power_log_file, CombatLogEvent, GameReplay, ReplayCard, ReplayUnit, TavernActionRecord,
    TurnReplay,
};

fn default_power_log_path() -> PathBuf {
    if let Ok(home) = env::var("HOME") {
        let candidate = PathBuf::from(home).join("scratch/Power.log");
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from("/usr/local/google/home/maxnelso/scratch/Power.log")
}

fn print_usage() {
    println!("Usage:");
    println!("  cargo run --bin replay_cli -- list [<Power.log>]");
    println!("  cargo run --bin replay_cli -- summary [<Power.log>] [--game <N>]");
    println!("  cargo run --bin replay_cli -- turn <TURN> [<Power.log>] [--game <N>]");
    println!(
        "  cargo run --bin replay_cli -- export-yaml [<Power.log>] [--game <N>] [--turn <T>] [-o <file.yaml>]"
    );
}

fn fmt_units(units: &[ReplayUnit]) -> String {
    if units.is_empty() {
        "(empty)".to_string()
    } else {
        units
            .iter()
            .map(|u| u.display_short())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn fmt_cards(cards: &[ReplayCard]) -> String {
    if cards.is_empty() {
        "(empty)".to_string()
    } else {
        cards
            .iter()
            .map(|c| c.display_short())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn fmt_outcome(outcome: BattleOutcome, hero_dmg: i32) -> String {
    match outcome {
        BattleOutcome::AWin => format!("WIN (+{hero_dmg} dmg)"),
        BattleOutcome::BWin => format!("LOSS ({hero_dmg} dmg)"),
        BattleOutcome::Draw => "TIE (0 dmg)".to_string(),
    }
}

fn print_game_list(path: &Path, games: &[GameReplay]) {
    println!("================================================================================");
    println!(
        " POWER.LOG GAMES ({}) — {} game(s) found",
        path.display(),
        games.len()
    );
    println!("================================================================================");
    for g in games {
        let mode = if g.is_duos { "Duos" } else { "Solo" };
        let last_turn = g.turns.last().map(|t| t.turn).unwrap_or(0);
        let end_hp = g
            .turns
            .last()
            .map(|t| format!("{}+{} HP", t.tavern.hero_health, t.tavern.hero_armor))
            .unwrap_or_else(|| "?".to_string());
        println!(
            "  Game #{:<2} | {} | {:<4} | Player: {:<12} | Hero: {:<22} | {} turns (last: {})",
            g.game_index, g.start_time, mode, g.player_name, g.hero_name, last_turn, end_hp
        );
    }
    println!("================================================================================");
}

fn print_game_summary(g: &GameReplay) {
    let mode = if g.is_duos { "Duos" } else { "Solo" };
    println!("\n================================================================================");
    println!(
        " GAME #{} SUMMARY — {} ({}) | Hero: {} ({}) | Mode: {}",
        g.game_index, g.player_name, g.start_time, g.hero_name, g.hero_card_id, mode
    );
    println!("================================================================================");

    for t in &g.turns {
        let upg = t
            .tavern
            .upgrade_cost
            .map(|c| format!("upg:{c}g"))
            .unwrap_or_else(|| "MAX".to_string());
        println!(
            "\n--- Turn {:>2} | Tier {} ({}) | Start Gold: {}g | Hero: {}+{} HP | Deity: {}/{} ---",
            t.turn,
            t.tavern.tavern_tier,
            upg,
            t.tavern.starting_gold,
            t.tavern.hero_health,
            t.tavern.hero_armor,
            t.tavern.deity_attack,
            t.tavern.deity_health,
        );
        println!("  [Tavern] Start Shop : {}", fmt_cards(&t.tavern.initial_shop));
        let mut action_summaries = Vec::new();
        for act in &t.tavern.actions {
            let s = match act {
                TavernActionRecord::BuyMinion { card, .. } => format!("Buy({card})"),
                TavernActionRecord::BuySpell { spell, .. } => format!("BuySpell({spell})"),
                TavernActionRecord::PlayMinion { card, target, .. } => {
                    if let Some(tgt) = target {
                        format!("Play({card} -> {tgt})")
                    } else {
                        format!("Play({card})")
                    }
                }
                TavernActionRecord::CastSpell { spell, target, .. } => {
                    if let Some(tgt) = target {
                        format!("Cast({spell} -> {tgt})")
                    } else {
                        format!("Cast({spell})")
                    }
                }
                TavernActionRecord::SellMinion { card, .. } => format!("Sell({card})"),
                TavernActionRecord::Refresh { .. } => "Refresh".to_string(),
                TavernActionRecord::Freeze { is_frozen } => {
                    if *is_frozen {
                        "Freeze".to_string()
                    } else {
                        "Unfreeze".to_string()
                    }
                }
                TavernActionRecord::UpgradeTier { new_tier, .. } => {
                    format!("Upgrade(T{new_tier})")
                }
                TavernActionRecord::Activate { card, target, .. } => {
                    if let Some(tgt) = target {
                        format!("Activate({card} -> {tgt})")
                    } else {
                        format!("Activate({card})")
                    }
                }
                TavernActionRecord::HeroPower { card, .. } => format!("HeroPower({card})"),
                TavernActionRecord::Triple { golden_card } => format!("★TRIPLE({golden_card})"),
                TavernActionRecord::PassCard { card, .. } => format!("Pass({card})"),
            };
            action_summaries.push(s);
        }
        println!(
            "  [Tavern] Actions ({:>2}): {}",
            t.tavern.actions.len(),
            if action_summaries.is_empty() {
                "(none)".to_string()
            } else {
                action_summaries.join(" -> ")
            }
        );
        println!("  [Tavern] End Board  : {}", fmt_units(&t.tavern.final_board));

        if let Some(ref c) = t.combat {
            println!(
                "  [Combat] Matchup    : {} [T{}]  vs  {} [T{}]",
                c.friendly_hero, c.friendly_tier, c.opponent_hero, c.opponent_tier
            );
            println!("  [Combat] Side A Brd : {}", fmt_units(&c.team_a));
            println!("  [Combat] Side B Brd : {}", fmt_units(&c.team_b));

            let (board_a, board_b, unsupported) = c.to_seaglass_boards();
            let state = GameState {
                hero_tier_a: c.friendly_tier.max(1),
                hero_tier_b: c.opponent_tier.max(1),
                ..GameState::default()
            };
            let dist = simulate_batch(&board_a, &board_b, &state, 2026, 2000);
            let tag_ins = c
                .events
                .iter()
                .filter(|e| matches!(e, CombatLogEvent::TagIn { .. }))
                .count();
            let note = if tag_ins > 0 {
                format!(" (Duos: {tag_ins} tag-in(s) occurred)")
            } else if !unsupported.is_empty() {
                format!(" (vanilla-approx for: {})", unsupported.join(", "))
            } else {
                " (100% native Tier 1 catalog)".to_string()
            };
            println!(
                "  [Combat] Result     : {:<16} | Seaglass 1v1 Sim: Win {:>5.1}% / Tie {:>5.1}% / Loss {:>5.1}%{}",
                fmt_outcome(c.outcome, c.hero_damage),
                dist.a_win_rate * 100.0,
                dist.draw_rate * 100.0,
                dist.b_win_rate * 100.0,
                note
            );
        }
    }
    println!("\n================================================================================");
}

fn print_turn_detail(g: &GameReplay, t: &TurnReplay) {
    let upg = t
        .tavern
        .upgrade_cost
        .map(|c| format!("{c}g"))
        .unwrap_or_else(|| "MAX".to_string());
    println!("\n================================================================================");
    println!(
        " GAME #{} ({}) — TURN {} DEEP DIVE",
        g.game_index, g.hero_name, t.turn
    );
    println!("================================================================================");
    println!(
        " [TAVERN PHASE] Tier {} (Upgrade: {}) | Starting Gold: {}g | Hero: {}+{} HP | Deity: {}/{}",
        t.tavern.tavern_tier,
        upg,
        t.tavern.starting_gold,
        t.tavern.hero_health,
        t.tavern.hero_armor,
        t.tavern.deity_attack,
        t.tavern.deity_health
    );
    println!("--------------------------------------------------------------------------------");
    println!("  Initial Board : {}", fmt_units(&t.tavern.initial_board));
    println!("  Initial Hand  : {}", fmt_cards(&t.tavern.initial_hand));
    println!("  Initial Shop  : {}", fmt_cards(&t.tavern.initial_shop));
    println!("\n  Chronological Tavern Actions ({}):", t.tavern.actions.len());
    if t.tavern.actions.is_empty() {
        println!("    (no actions taken)");
    }
    for (i, act) in t.tavern.actions.iter().enumerate() {
        let step_no = i + 1;
        match act {
            TavernActionRecord::BuyMinion {
                card,
                shop_pos,
                gold_after,
            } => {
                let pos_str = shop_pos
                    .map(|p| format!(" [slot {p}]"))
                    .unwrap_or_default();
                println!(
                    "    {step_no:>2}. [Buy Minion]  {card}{pos_str}  (gold left: {gold_after}g)"
                );
            }
            TavernActionRecord::BuySpell { spell, gold_after } => {
                println!("    {step_no:>2}. [Buy Spell]   {spell}  (gold left: {gold_after}g)");
            }
            TavernActionRecord::PlayMinion {
                card,
                board_pos,
                target,
                gold_after,
            } => {
                let pos_str = board_pos
                    .map(|p| format!(" at board[{p}]"))
                    .unwrap_or_default();
                let tgt_str = target
                    .as_ref()
                    .map(|tg| format!(" -> target: {tg}"))
                    .unwrap_or_default();
                println!(
                    "    {step_no:>2}. [Play Minion] {card}{pos_str}{tgt_str}  (gold left: {gold_after}g)"
                );
            }
            TavernActionRecord::CastSpell {
                spell,
                target,
                gold_after,
            } => {
                let tgt_str = target
                    .as_ref()
                    .map(|tg| format!(" -> target: {tg}"))
                    .unwrap_or_default();
                println!(
                    "    {step_no:>2}. [Cast Spell]  {spell}{tgt_str}  (gold left: {gold_after}g)"
                );
            }
            TavernActionRecord::SellMinion { card, gold_after } => {
                println!("    {step_no:>2}. [Sell Minion] {card}  (gold left: {gold_after}g)");
            }
            TavernActionRecord::Refresh {
                gold_after,
                new_shop,
            } => {
                println!(
                    "    {step_no:>2}. [Refresh]     (gold left: {gold_after}g) -> Rolled: {}",
                    fmt_cards(new_shop)
                );
            }
            TavernActionRecord::Freeze { is_frozen } => {
                let label = if *is_frozen { "Freeze" } else { "Unfreeze" };
                println!("    {step_no:>2}. [{label}]");
            }
            TavernActionRecord::UpgradeTier {
                new_tier,
                gold_after,
            } => {
                println!(
                    "    {step_no:>2}. [Upgrade]     Tavern Tier -> {new_tier}  (gold left: {gold_after}g)"
                );
            }
            TavernActionRecord::Activate {
                card,
                target,
                gold_after,
            } => {
                let tgt_str = target
                    .as_ref()
                    .map(|tg| format!(" -> target: {tg}"))
                    .unwrap_or_default();
                println!(
                    "    {step_no:>2}. [Activate]    {card}{tgt_str}  (gold left: {gold_after}g)"
                );
            }
            TavernActionRecord::HeroPower {
                card,
                target,
                gold_after,
            } => {
                let tgt_str = target
                    .as_ref()
                    .map(|tg| format!(" -> target: {tg}"))
                    .unwrap_or_default();
                println!(
                    "    {step_no:>2}. [Hero Power]  {card}{tgt_str}  (gold left: {gold_after}g)"
                );
            }
            TavernActionRecord::Triple { golden_card } => {
                println!("    {step_no:>2}. [★ TRIPLE ★]  Combined into Golden {golden_card}!");
            }
            TavernActionRecord::PassCard { card, gold_after } => {
                println!(
                    "    {step_no:>2}. [Pass Portal] Passed {card} to teammate  (gold left: {gold_after}g)"
                );
            }
        }
    }
    println!("\n  End-of-Tavern Snapshot:");
    println!("    Final Board : {}", fmt_units(&t.tavern.final_board));
    println!("    Final Hand  : {}", fmt_cards(&t.tavern.final_hand));
    println!(
        "    Final Shop  : {}{}",
        fmt_cards(&t.tavern.final_shop),
        if t.tavern.is_frozen { " [FROZEN]" } else { "" }
    );
    println!("    Ending Gold : {}g", t.tavern.ending_gold);

    let Some(ref c) = t.combat else {
        println!("\n [COMBAT PHASE] (No combat recorded for this turn)");
        return;
    };

    println!("\n================================================================================");
    println!(
        " [COMBAT PHASE] {} [Tier {}]  vs  {} [Tier {}]",
        c.friendly_hero, c.friendly_tier, c.opponent_hero, c.opponent_tier
    );
    println!("================================================================================");
    println!("  Side A Starting Board ({} minions):", c.team_a.len());
    if c.team_a.is_empty() {
        println!("    (empty)");
    }
    for (i, u) in c.team_a.iter().enumerate() {
        println!(
            "    [{i}] #{:<5} {:<34} (spec: {:?})",
            u.entity_id,
            u.display_short(),
            u.to_scenario_spec()
        );
    }
    println!("  Side B Starting Board ({} minions):", c.team_b.len());
    if c.team_b.is_empty() {
        println!("    (empty)");
    }
    for (i, u) in c.team_b.iter().enumerate() {
        println!(
            "    [{i}] #{:<5} {:<34} (spec: {:?})",
            u.entity_id,
            u.display_short(),
            u.to_scenario_spec()
        );
    }

    println!("\n  Server Combat Event Log ({} events):", c.events.len());
    for (idx, ev) in c.events.iter().enumerate() {
        let step = idx + 1;
        match ev {
            CombatLogEvent::Attack {
                side,
                attacker,
                defender,
                attacker_after,
                defender_after,
            } => {
                println!(
                    "    {step:>2}. [Attack] ({side}) {attacker} --> {defender}  (after: {attacker_after} vs {defender_after})"
                );
            }
            CombatLogEvent::Trigger { source, summary } => {
                println!("    {step:>2}. [Trigger] {source}: {summary}");
            }
            CombatLogEvent::Deaths { units } => {
                println!("    {step:>2}. [Deaths]  ☠ {}", units.join(", "));
            }
            CombatLogEvent::TagIn { side, hero, board } => {
                println!(
                    "    {step:>2}. [Tag-In]  >>> {side} ({hero}) tags in with: {} <<<",
                    fmt_units(board)
                );
            }
            CombatLogEvent::HeroDamage {
                attacker_hero,
                defender_hero,
                damage,
            } => {
                println!(
                    "    {step:>2}. [HeroHit] {attacker_hero} hits {defender_hero} for {damage} damage!"
                );
            }
        }
    }

    println!("\n  Server Combat Result: {}", fmt_outcome(c.outcome, c.hero_damage));
    println!("    Side A Survivors: {}", fmt_units(&c.survivors_a));
    println!("    Side B Survivors: {}", fmt_units(&c.survivors_b));

    let (board_a, board_b, unsupported) = c.to_seaglass_boards();
    let state = GameState {
        hero_tier_a: c.friendly_tier.max(1),
        hero_tier_b: c.opponent_tier.max(1),
        ..GameState::default()
    };
    let dist = simulate_batch(&board_a, &board_b, &state, 2026, 10000);
    println!("--------------------------------------------------------------------------------");
    println!(
        "  Seaglass 1v1 Monte Carlo (10,000 rollouts on initial starting boards):"
    );
    println!(
        "    Side A Win: {:>6.2}%  |  Tie: {:>6.2}%  |  Side B Win: {:>6.2}%  |  Mean Hero Dmg: {:+.2}",
        dist.a_win_rate * 100.0,
        dist.draw_rate * 100.0,
        dist.b_win_rate * 100.0,
        dist.damage.mean
    );
    if !unsupported.is_empty() {
        println!(
            "    Note: Unimplemented card triggers simulated as stat/keyword bodies: {}",
            unsupported.join(", ")
        );
    }
    println!("================================================================================");
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut cmd = "summary".to_string();
    let mut log_path: Option<PathBuf> = None;
    let mut game_num: usize = 1;
    let mut turn_num: Option<u32> = None;
    let mut out_path: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" | "help" => {
                print_usage();
                return;
            }
            "list" | "summary" | "turn" | "export-yaml" => {
                cmd = args[i].clone();
                if cmd == "turn" && i + 1 < args.len() {
                    if let Ok(t) = args[i + 1].parse::<u32>() {
                        turn_num = Some(t);
                        i += 1;
                    }
                }
            }
            "--game" | "-g" => {
                i += 1;
                game_num = args
                    .get(i)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or_else(|| {
                        eprintln!("--game requires a 1-indexed game number");
                        process::exit(1);
                    });
            }
            "--turn" | "-t" => {
                i += 1;
                turn_num = Some(
                    args.get(i)
                        .and_then(|s| s.parse::<u32>().ok())
                        .unwrap_or_else(|| {
                            eprintln!("--turn requires a turn number");
                            process::exit(1);
                        }),
                );
            }
            "--out" | "-o" => {
                i += 1;
                out_path = Some(PathBuf::from(args.get(i).unwrap_or_else(|| {
                    eprintln!("--out requires a file path");
                    process::exit(1);
                })));
            }
            other => {
                if turn_num.is_none() && cmd == "turn" {
                    if let Ok(t) = other.parse::<u32>() {
                        turn_num = Some(t);
                        i += 1;
                        continue;
                    }
                }
                log_path = Some(PathBuf::from(other));
            }
        }
        i += 1;
    }

    let path = log_path.unwrap_or_else(default_power_log_path);
    let games = parse_power_log_file(&path).unwrap_or_else(|e| {
        eprintln!("Error parsing {}: {e}", path.display());
        process::exit(1);
    });

    if games.is_empty() {
        eprintln!("No Battlegrounds games found in {}", path.display());
        process::exit(1);
    }

    if cmd == "list" {
        print_game_list(&path, &games);
        return;
    }

    let game = games
        .iter()
        .find(|g| g.game_index == game_num)
        .unwrap_or_else(|| {
            eprintln!(
                "Game #{game_num} not found (file contains {} game(s))",
                games.len()
            );
            process::exit(1);
        });

    match cmd.as_str() {
        "summary" => {
            print_game_list(&path, &games);
            print_game_summary(game);
        }
        "turn" => {
            let want_turn = turn_num.unwrap_or(1);
            let turn = game
                .turns
                .iter()
                .find(|t| t.turn == want_turn)
                .unwrap_or_else(|| {
                    eprintln!("Turn {want_turn} not found in Game #{game_num}");
                    process::exit(1);
                });
            print_turn_detail(game, turn);
        }
        "export-yaml" => {
            let yaml = if let Some(want_turn) = turn_num {
                let turn = game
                    .turns
                    .iter()
                    .find(|t| t.turn == want_turn)
                    .unwrap_or_else(|| {
                        eprintln!("Turn {want_turn} not found in Game #{game_num}");
                        process::exit(1);
                    });
                let combat = turn.combat.as_ref().unwrap_or_else(|| {
                    eprintln!("Turn {want_turn} has no combat phase");
                    process::exit(1);
                });
                let scenario = combat.to_scenario(want_turn);
                serde_yaml::to_string(&scenario).unwrap()
            } else {
                serde_yaml::to_string(game).unwrap()
            };

            if let Some(out) = out_path {
                fs::write(&out, &yaml).unwrap_or_else(|e| {
                    eprintln!("Failed to write {}: {e}", out.display());
                    process::exit(1);
                });
                println!("Wrote YAML to {}", out.display());
            } else {
                print!("{yaml}");
            }
        }
        _ => print_usage(),
    }
}
