//! Head-to-head 2-player Battlegrounds game simulator and full-trace logger.
//!
//! Runs a complete game between two semi-random players sharing a `CardPool` and deterministic `Rng`,
//! printing every Tavern action, state transition, aura update, and Combat event. The players see
//! each other through the shared lobby (`seaglass::lobby::Lobby`, `engine/docs/tavern.md` §3.1):
//! each is the other's next and last opponent, and their lobby gifts reach each other. The game runs
//! as a recorded `Match`, so `--record` can save its replay (`engine/docs/replays.md`); the bots
//! draw from their own RNG, never the engine's.
//!
//! Usage:
//!   cargo run --bin sim_match
//!   cargo run --bin sim_match -- 123
//!   cargo run --bin sim_match -- --seed 7 --max-turns 20
//!   cargo run --bin sim_match -- --seed 7 --quiet --record replays/s7.replay.pb

use std::env;
use std::panic::{self, AssertUnwindSafe};
use std::process::ExitCode;

use seaglass::{
    full_catalog, DeityKind, Match, MatchSetup, PlayerSetup, PoolMode, Rng, TavernAction,
};
use seaglass_tools::bot::choose_semi_random_action;
use seaglass_tools::fmt::{
    describe_action, print_combat_trace, print_player_snapshot, print_step_diff,
};

/// Mixed into the match seed to seed the bots' RNG.
const BOT_SEED_SALT: u64 = 0xB075_EED5_0F5E_A61A;

const LABELS: [&str; 2] = ["P1", "P2"];

fn run_player_tavern_turn(game: &mut Match, p: usize, bot: &mut Rng, quiet: bool) {
    let label = LABELS[p];
    if !quiet {
        println!(
            "\n  >>> {label} TAVERN PHASE (Turn {}) <<<",
            game.player(p).turn
        );
        print_player_snapshot(label, game.player(p), game.pool(p));
    }

    let max_steps = 40usize;
    for step_num in 1..=max_steps {
        let state = game.player(p);
        let action = if step_num == max_steps && state.discover_pending.is_none() {
            TavernAction::EndTurn
        } else {
            choose_semi_random_action(state, bot)
        };

        let before = state.clone();
        if !quiet {
            let desc = describe_action(&before, &action);
            println!("    [{label} Step {step_num:02}] {desc}");
        }

        match game.step(p, action) {
            Ok(ended) => {
                if !quiet {
                    print_step_diff(&before, game.player(p), game.pool(p));
                }
                if ended {
                    break;
                }
            }
            Err(err) => {
                println!("      !! ERROR executing {action:?}: {err}");
                break;
            }
        }
    }
}

/// Play the match to the end (a knockout or `max_turns`).
fn play(game: &mut Match, bot: &mut Rng, max_turns: u32, quiet: bool) {
    for turn in 1..=max_turns {
        if !quiet {
            println!("\n################################################################################");
            println!("                              TURN {turn} BEGINS");
            println!(
                "################################################################################"
            );
        }

        // Each is the other's next opponent (head to head: no featured pair to wager on).
        game.set_pairs(&[(0, 1)]);
        game.start_turn(0);
        game.start_turn(1);
        game.deliver_gifts();

        run_player_tavern_turn(game, 0, bot, quiet);
        run_player_tavern_turn(game, 1, bot, quiet);
        game.deliver_gifts();

        let pre_a = game.player(0).clone();
        let pre_b = game.player(1).clone();
        let res = game.combat(0, 1);
        game.record_deaths();
        if !quiet {
            print_combat_trace(
                turn,
                LABELS,
                &pre_a,
                &pre_b,
                &res,
                game.player(0),
                game.player(1),
                game.pool(0),
            );
        }

        let (p1, p2) = (game.player(0), game.player(1));
        if p1.health <= 0 || p2.health <= 0 {
            let result = if p1.health <= 0 && p2.health <= 0 {
                format!("GAME OVER ON TURN {turn}: DOUBLE KNOCKOUT!")
            } else if p2.health <= 0 {
                format!(
                    "GAME OVER ON TURN {turn}: P1 WINS! (P1 HP: {}+{}, P2 HP: {})",
                    p1.health, p1.armor, p2.health
                )
            } else {
                format!(
                    "GAME OVER ON TURN {turn}: P2 WINS! (P2 HP: {}+{}, P1 HP: {})",
                    p2.health, p2.armor, p1.health
                )
            };
            game.finish();
            print_result(&result, quiet);
            return;
        }
    }

    let (p1, p2) = (game.player(0), game.player(1));
    let result = format!(
        "REACHED MAX TURNS ({max_turns}): P1 HP {}+{} vs P2 HP {}+{}",
        p1.health, p1.armor, p2.health, p2.armor
    );
    game.finish();
    print_result(&result, quiet);
}

fn print_result(result: &str, quiet: bool) {
    if quiet {
        println!("{result}");
    } else {
        println!(
            "\n################################################################################"
        );
        println!(" {result}");
        println!(
            "################################################################################"
        );
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut seed: u64 = 42;
    let mut max_turns: u32 = 20;
    let mut shop_spells = true;
    let mut quiet = false;
    let mut record: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" | "-s" => {
                if let Some(val) = args.get(i + 1).and_then(|s| s.parse::<u64>().ok()) {
                    seed = val;
                    i += 1;
                }
            }
            "--max-turns" | "-t" => {
                if let Some(val) = args.get(i + 1).and_then(|s| s.parse::<u32>().ok()) {
                    max_turns = val;
                    i += 1;
                }
            }
            "--no-spells" => {
                shop_spells = false;
            }
            "--quiet" | "-q" => {
                quiet = true;
            }
            "--record" | "-r" => {
                let Some(path) = args.get(i + 1) else {
                    eprintln!("--record needs a file path");
                    return ExitCode::FAILURE;
                };
                record = Some(path.clone());
                i += 1;
            }
            other => {
                if let Ok(val) = other.parse::<u64>() {
                    seed = val;
                }
            }
        }
        i += 1;
    }

    let player = |name: &str, deity| PlayerSetup {
        name: name.to_string(),
        deity,
        armor: 5,
        shop_spells,
        // As `TavernState::new()`: no trinkets, no Dark Gifts.
        trinkets: false,
        dark_gifts: false,
        ..Default::default()
    };
    let setup = MatchSetup {
        catalog: "full".to_string(),
        pool: PoolMode::Shared,
        shared_seed: seed,
        players: vec![
            player(LABELS[0], DeityKind::CThun),
            player(LABELS[1], DeityKind::YShaarj),
        ],
        meta: [
            ("source".to_string(), "sim_match".to_string()),
            ("max_turns".to_string(), max_turns.to_string()),
        ]
        .into(),
    };
    let mut game = Match::new(&setup).expect("the full catalog exists");
    let mut bot = Rng::new(seed ^ BOT_SEED_SALT);

    if !quiet {
        println!(
            "================================================================================"
        );
        println!(
            " SEAGLASS 2-PLAYER FULL MATCH TRACE | Seed: {seed} | Max Turns: {max_turns} | Catalog: {} Solo Minions | Shop Spells: {shop_spells}",
            full_catalog().len()
        );
        println!(" Player 1 (P1): HP 30 + 5 Armor, Deity = C'Thun");
        println!(" Player 2 (P2): HP 30 + 5 Armor, Deity = Y'Shaarj");
        println!(
            "================================================================================"
        );
    } else {
        print!("seed {seed}: ");
    }

    // Save the replay even if the engine panics: it then ends with the call that panicked.
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        play(&mut game, &mut bot, max_turns, quiet)
    }));
    if let Some(path) = &record {
        match game.save(path) {
            Ok(()) => eprintln!("replay saved to {path} ({} calls)", game.records().len()),
            Err(err) => {
                eprintln!("could not save the replay to {path}: {err}");
                return ExitCode::FAILURE;
            }
        }
    }
    if let Err(payload) = outcome {
        panic::resume_unwind(payload);
    }
    ExitCode::SUCCESS
}
