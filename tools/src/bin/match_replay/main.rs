//! Inspect and verify match replays (`engine/docs/replays.md`).
//!
//! Usage:
//!   match_replay summary <file>                      players, every combat and the result
//!   match_replay log <file>                          the whole game, call by call
//!   match_replay show <file> [--frame N | --turn T]  every player's full state at a frame
//!   match_replay step <file> [--frame N | --turn T]  step through the game interactively
//!   match_replay tui <file> [--frame N | --turn T]   play the game back in a terminal UI
//!   match_replay cast <file> [-o out.cast]           record that play-back for asciinema
//!   match_replay compare <Power.log> [--game G]      a Power.log game's combats vs the engine
//!   match_replay verify <file>...                    re-simulate replays; fail if any diverges
//!   match_replay html <file> [-o out.html]           a self-contained HTML viewer
//!   match_replay scenario <file> (--frame N | --turn T) [--steps K] [--player P]
//!                                                    a scenario test reproducing a frame

mod html;
mod scenario;
mod tui;

use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use seaglass::replay::FrameKind;
use seaglass::Timeline;
use seaglass_tools::replay_text::{print_frame, print_problems, print_state, print_summary};

const USAGE: &str = "\
usage: match_replay <command> <file> [options]

commands:
  summary <file>                       players, every combat and the result
  log <file>                           the whole game, call by call, with combat logs
  show <file> [--frame N | --turn T]   every player's full state at a frame (default: last)
  step <file> [--frame N | --turn T]   step through the game interactively
  tui <file> [--frame N | --turn T]    play the game back in a terminal UI, combats strike by
                                       strike (`match_replay tui --help` for its options)
  cast <file> [-o out.cast]            record that play-back as an asciinema recording
                                       (`match_replay cast --help` for its options)
  compare <Power.log> [--game G] [--turn T] [--full]
                                       every combat of a Power.log game next to the engine's
                                       version: results and the first strike the boards differ
  verify <file>...                     re-simulate replays; exit 1 if any diverges
  html <file> [-o out.html]            write a self-contained HTML viewer
  scenario <file> (--frame N | --turn T) [--steps K] [--player P] [-o out.yaml]
                                       write a scenario test that reproduces a frame";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = args.split_first() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let result = match command.as_str() {
        "summary" => load_one(rest).map(|timeline| {
            print_summary(&timeline);
            ExitCode::SUCCESS
        }),
        "log" => load_one(rest).map(|timeline| {
            for i in 0..timeline.len() {
                print_frame(&timeline, i, true);
            }
            print_problems(&timeline);
            ExitCode::SUCCESS
        }),
        "show" => load_one(rest).and_then(|timeline| {
            let i = frame_arg(&timeline, rest)?.unwrap_or(timeline.len() - 1);
            print_state(&timeline, i);
            Ok(ExitCode::SUCCESS)
        }),
        "step" => load_one(rest).and_then(|timeline| {
            let i = frame_arg(&timeline, rest)?.unwrap_or(0);
            step(&timeline, i).map_err(|e| e.to_string())?;
            Ok(ExitCode::SUCCESS)
        }),
        "tui" if rest.iter().any(|a| a == "--help" || a == "-h") => {
            println!("{}", tui::USAGE);
            Ok(ExitCode::SUCCESS)
        }
        "tui" => tui::run(rest),
        "cast" if rest.iter().any(|a| a == "--help" || a == "-h") => {
            println!("{}", tui::cast::USAGE);
            Ok(ExitCode::SUCCESS)
        }
        "cast" => tui::cast::run(rest),
        "compare" => tui::compare(rest),
        "verify" => verify(rest),
        "html" => html::run(rest),
        "scenario" => scenario::run(rest),
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        other => Err(format!("unknown command {other:?}\n\n{USAGE}")),
    };
    result.unwrap_or_else(|message| {
        eprintln!("{message}");
        ExitCode::FAILURE
    })
}

/// Load and re-simulate the replay named by the first argument.
pub(crate) fn load_one(args: &[String]) -> Result<Timeline, String> {
    let path = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| format!("missing the replay file\n\n{USAGE}"))?;
    let timeline = Timeline::load(path)?;
    if let Some(engine) = timeline.engine_mismatch() {
        eprintln!(
            "warning: {path} was recorded by seaglass {engine}, not this build ({}); \
             states may differ from the recorded game",
            seaglass::replay::engine_version()
        );
    }
    if !timeline.is_faithful() {
        eprintln!("warning: {path} does not re-simulate faithfully:");
        for d in &timeline.divergences {
            eprintln!("  record {} (frame {}): {}", d.record, d.frame, d.message);
        }
        if let Some(e) = &timeline.error {
            eprintln!("  stopped at record {}: {}", e.record, e.message);
        }
    }
    Ok(timeline)
}

/// The value of option `name` (e.g. `--frame 12`), if given.
pub(crate) fn option<'a>(args: &'a [String], names: &[&str]) -> Option<&'a str> {
    args.iter()
        .position(|a| names.contains(&a.as_str()))
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

/// The frame chosen by `--frame N` or `--turn T`, if either is given.
pub(crate) fn frame_arg(timeline: &Timeline, args: &[String]) -> Result<Option<usize>, String> {
    if let Some(frame) = option(args, &["--frame", "-f"]) {
        let i: usize = frame
            .parse()
            .map_err(|_| format!("--frame needs a number, not {frame:?}"))?;
        if i >= timeline.len() {
            return Err(format!(
                "frame {i} does not exist (the replay has frames 0..={})",
                timeline.len() - 1
            ));
        }
        return Ok(Some(i));
    }
    if let Some(turn) = option(args, &["--turn", "-t"]) {
        let t: u32 = turn
            .parse()
            .map_err(|_| format!("--turn needs a number, not {turn:?}"))?;
        return timeline.turn_start(t).map(Some).ok_or_else(|| {
            format!(
                "turn {t} does not exist (the replay reaches turn {})",
                timeline.last_turn()
            )
        });
    }
    Ok(None)
}

fn verify(paths: &[String]) -> Result<ExitCode, String> {
    if paths.is_empty() {
        return Err(format!("missing the replay files\n\n{USAGE}"));
    }
    let mut failed = 0;
    for path in paths {
        match Timeline::load(path) {
            Ok(timeline) if timeline.is_faithful() => {
                let ended = matches!(
                    timeline.frames.last().map(|f| &f.kind),
                    Some(FrameKind::End { .. })
                );
                println!(
                    "ok    {path}: {} frames, {} turns{}",
                    timeline.len(),
                    timeline.last_turn(),
                    if ended {
                        ""
                    } else {
                        " (ends before the match does)"
                    }
                );
            }
            Ok(timeline) => {
                failed += 1;
                println!("FAIL  {path}");
                print_problems(&timeline);
            }
            Err(message) => {
                failed += 1;
                println!("FAIL  {path}: {message}");
            }
        }
    }
    if failed > 0 {
        println!("{failed} of {} replays failed", paths.len());
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

const STEP_HELP: &str = "\
  enter/n  next frame          p  previous frame
  N        next turn           P  previous turn (or this turn's start)
  g I      go to frame I       t T  go to turn T
  s        every player's full state
  c        this combat's event log
  d        this frame again    q  quit";

/// Step through a timeline interactively, starting at frame `start`.
fn step(timeline: &Timeline, start: usize) -> io::Result<()> {
    let last = timeline.len() - 1;
    let mut i = start;
    println!("{STEP_HELP}\n");
    print_frame(timeline, i, false);
    let stdin = io::stdin();
    let mut line = String::new();
    loop {
        print!("[{i}/{last}] > ");
        io::stdout().flush()?;
        line.clear();
        if stdin.lock().read_line(&mut line)? == 0 {
            println!();
            return Ok(());
        }
        let mut words = line.split_whitespace();
        let command = words.next().unwrap_or("n");
        let arg = words.next();
        let turn = timeline.frames[i].turn;
        let target = match command {
            "n" => Some((i + 1).min(last)),
            "p" => Some(i.saturating_sub(1)),
            "N" => Some(
                (i + 1..=last)
                    .find(|&j| timeline.frames[j].turn > turn)
                    .unwrap_or(last),
            ),
            "P" => {
                let start = timeline.turn_start(turn).unwrap_or(0);
                if start < i {
                    Some(start)
                } else {
                    Some(timeline.turn_start(turn.saturating_sub(1)).unwrap_or(0))
                }
            }
            "g" => match arg.and_then(|a| a.parse::<usize>().ok()) {
                Some(j) if j <= last => Some(j),
                _ => {
                    println!("g needs a frame number in 0..={last}");
                    None
                }
            },
            "t" => match arg
                .and_then(|a| a.parse::<u32>().ok())
                .and_then(|t| timeline.turn_start(t))
            {
                Some(j) => Some(j),
                None => {
                    println!("t needs a turn in 0..={}", timeline.last_turn());
                    None
                }
            },
            "s" => {
                print_state(timeline, i);
                None
            }
            "c" => {
                if matches!(timeline.frames[i].kind, FrameKind::Combat { .. }) {
                    print_frame(timeline, i, true);
                } else {
                    println!("frame {i} is not a combat");
                }
                None
            }
            "d" => Some(i),
            "q" => return Ok(()),
            _ => {
                println!("{STEP_HELP}");
                None
            }
        };
        if let Some(j) = target {
            if j == i && command == "n" {
                println!("(the last frame)");
            } else {
                i = j;
                print_frame(timeline, i, false);
            }
        }
    }
}
