//! `match_replay scenario` end to end: record a match, export frames of it as scenarios, run
//! them.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use seaglass::replay::FrameKind;
use seaglass::scenario::ScenarioFile;
use seaglass::{DeityKind, Match, MatchSetup, PlayerSetup, PoolMode, Rng, TavernAction, Timeline};
use seaglass_tools::bot::choose_semi_random_action;

/// An empty directory for one test's files.
fn scratch_dir(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("match_replay_scenario")
        .join(test);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Player `p`'s `step`th action of their turn (0-based) by `sim_match`'s bot, who ends the turn
/// after 40, picks a trinket when it is offered some and, one time in four, uses its Hero Power
/// or the Dark Discovery button when it can; whether it ends their turn.
fn bot_action(game: &mut Match, bot: &mut Rng, p: usize, step: usize) -> bool {
    let state = game.player(p);
    let powers: Vec<TavernAction> = state
        .valid_actions()
        .into_iter()
        .filter(|a| {
            matches!(
                a,
                TavernAction::HeroPower { .. }
                    | TavernAction::HeroPower2 { .. }
                    | TavernAction::DarkDiscovery
            )
        })
        .collect();
    let action = if !state.trinket_options.is_empty() {
        TavernAction::ChooseTrinket {
            option_index: bot.below(state.trinket_options.len()),
        }
    } else if step >= 39 && state.discover_pending.is_none() {
        TavernAction::EndTurn
    } else if !powers.is_empty() && bot.below(4) == 0 {
        powers[bot.below(powers.len())]
    } else {
        choose_semi_random_action(state, bot)
    };
    game.step(p, action) != Ok(false)
}

/// Record a whole match between `sim_match`'s bots (with heroes, trinkets and Dark Gifts) to
/// `path`, and re-simulate it. With a shared pool, each player makes their whole turn in turn (as
/// `sim_match` plays); with a pool per player, they alternate actions (as `train_ppo.py`'s
/// self-play does).
fn record_match(path: &Path, pool: PoolMode, seed: u64) -> Timeline {
    let heroes = seaglass::heroes::offerable(false, true);
    let player = |p: u64, deity| PlayerSetup {
        name: format!("P{}", p + 1),
        seed: seed * 10 + p,
        deity,
        armor: 5,
        shop_spells: true,
        hero: Some(heroes[((seed * 7 + p * 31) % heroes.len() as u64) as usize]),
        ..Default::default()
    };
    let mut game = Match::new(&MatchSetup {
        pool,
        shared_seed: seed,
        players: vec![player(0, DeityKind::CThun), player(1, DeityKind::YShaarj)],
        ..Default::default()
    })
    .unwrap();
    let mut bot = Rng::new(seed);
    for _turn in 0..20 {
        game.set_pairs(&[(0, 1)]);
        game.start_turn(0);
        game.start_turn(1);
        game.deliver_gifts();
        if pool == PoolMode::Shared {
            for p in 0..2 {
                for step in 0.. {
                    if bot_action(&mut game, &mut bot, p, step) {
                        break;
                    }
                }
            }
        } else {
            let mut active = vec![0, 1];
            for step in 0.. {
                active.retain(|&p| !bot_action(&mut game, &mut bot, p, step));
                if active.is_empty() {
                    break;
                }
            }
        }
        game.deliver_gifts();
        game.combat(0, 1);
        if game.record_deaths().contains(&false) {
            break;
        }
    }
    game.finish();
    game.save(path).unwrap();
    let timeline = Timeline::load(path).unwrap();
    assert!(timeline.is_faithful(), "{:?}", timeline.divergences);
    timeline
}

fn match_replay_scenario(replay: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_match_replay"))
        .arg("scenario")
        .arg(replay)
        .args(args)
        .output()
        .expect("match_replay runs")
}

/// The first action of turn `turn` or later, and who makes it.
fn action_from_turn(timeline: &Timeline, turn: u32) -> (usize, usize) {
    (1..timeline.len())
        .find_map(|i| match timeline.frames[i].kind {
            FrameKind::Act { player, .. } if timeline.frames[i].turn >= turn => Some((i, player)),
            _ => None,
        })
        .expect("an action from that turn on")
}

/// Run every scenario in `file`.
fn run_all(file: &ScenarioFile) {
    assert!(!file.scenarios.is_empty());
    for scenario in &file.scenarios {
        scenario
            .run()
            .unwrap_or_else(|failure| panic!("{}: {failure}", scenario.name));
    }
}

#[test]
fn scenario_writes_a_scenario_that_passes() {
    let dir = scratch_dir("written");
    let replay = dir.join("game.replay.pb");
    let timeline = record_match(&replay, PoolMode::Shared, 11);
    let (i, _) = action_from_turn(&timeline, 4);
    let file = dir.join("frame.yaml");
    let frame = (i - 1).to_string();
    let out = match_replay_scenario(
        &replay,
        &[
            "--frame",
            &frame,
            "--steps",
            "3",
            "-o",
            file.to_str().unwrap(),
        ],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(
        stderr.starts_with(&format!(
            "wrote {}: scenario game_frame_{frame}_",
            file.display()
        )),
        "{stderr}"
    );
    assert!(stderr.contains("it passes"), "{stderr}");
    assert!(out.stdout.is_empty());
    run_all(&ScenarioFile::load(&file).unwrap());
}

#[test]
fn scenario_prints_the_scenario_without_an_output_file() {
    let dir = scratch_dir("printed");
    let replay = dir.join("game.replay.pb");
    let timeline = record_match(&replay, PoolMode::PerPlayer, 12);
    let (i, player) = action_from_turn(&timeline, 3);
    let who = format!("P{}", player + 1);
    let turn = timeline.frames[i].turn.to_string();
    // `--turn` picks the frame before the player's first action of the turn.
    let out = match_replay_scenario(&replay, &["--turn", &turn, "--player", &who, "-k", "2"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    let yaml = String::from_utf8(out.stdout).unwrap();
    assert!(
        yaml.starts_with("# Exported by `match_replay scenario`"),
        "{yaml}"
    );
    let topic = format!("topic: replay game.replay.pb frame {} ({who})", i - 1);
    assert!(yaml.lines().any(|l| l == topic), "no {topic:?} in\n{yaml}");
    run_all(&ScenarioFile::parse(&yaml).unwrap());

    // `--steps 0` rebuilds any player's state at any frame.
    let out = match_replay_scenario(&replay, &["--frame", "0", "--steps", "0", "-p", "1"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    run_all(&ScenarioFile::parse(&String::from_utf8(out.stdout).unwrap()).unwrap());
}

#[test]
fn scenario_passes_over_the_other_players_calls_with_a_pool_per_player() {
    let dir = scratch_dir("interleaved");
    let replay = dir.join("game.replay.pb");
    let timeline = record_match(&replay, PoolMode::PerPlayer, 14);
    // A turn in which P1's first three actions have P2's in between.
    let (turn, frames) = (3..=timeline.last_turn())
        .find_map(|turn| {
            let frames: Vec<usize> = (1..timeline.len())
                .filter(|&i| {
                    timeline.frames[i].turn == turn
                        && matches!(timeline.frames[i].kind, FrameKind::Act { player: 0, .. })
                })
                .take(3)
                .collect();
            (frames.len() == 3 && frames[2] - frames[0] > 2).then_some((turn, frames))
        })
        .expect("a turn in which P1 makes three actions");
    let file = dir.join("turn.yaml");
    let out = match_replay_scenario(
        &replay,
        &[
            "--turn",
            &turn.to_string(),
            "--player",
            "P1",
            "--steps",
            "3",
            "-o",
            file.to_str().unwrap(),
        ],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    // All three (no note): P2's calls cannot touch P1's Tavern, pool or RNG.
    assert!(
        stderr.starts_with("wrote ") && stderr.ends_with("(3 actions; it passes)\n"),
        "{stderr}"
    );
    let yaml = std::fs::read_to_string(&file).unwrap();
    assert!(yaml.contains("\n# P2's calls in between ("), "{yaml}");
    for frame in frames {
        let step = format!("  # frame {frame}");
        assert!(yaml.contains(&step), "no {step:?} in\n{yaml}");
    }
    run_all(&ScenarioFile::load(&file).unwrap());
}

#[test]
fn scenario_explains_what_it_cannot_export() {
    let dir = scratch_dir("errors");
    let replay = dir.join("game.replay.pb");
    let timeline = record_match(&replay, PoolMode::Shared, 13);
    let last = (timeline.len() - 1).to_string();
    let cases: [(&[&str], &str); 5] = [
        (
            &["--frame", &last],
            &format!("frame {last} is the last frame"),
        ),
        (&["--frame", "0", "--bogus"], "unknown option \"--bogus\""),
        (&[], "which frame? pass --frame N or --turn T"),
        (&["--frame", "0", "--player", "P9"], "no player \"P9\""),
        (&["--frame", "0", "--steps", "x"], "--steps needs a number"),
    ];
    for (args, message) in cases {
        let out = match_replay_scenario(&replay, args);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(1), "{args:?}: {stderr}");
        assert!(stderr.contains(message), "{args:?}: {stderr}");
        assert!(out.stdout.is_empty(), "{args:?}");
    }
    let out = match_replay_scenario(&replay, &["--help"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("usage: match_replay scenario"));
}
