//! `match_replay html` end to end: record a match, write its viewer page, read the page back.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use seaglass::replay::{self, pb};
use seaglass::{DeityKind, Match, MatchSetup, PlayerSetup, PoolMode, Rng, TavernAction, Timeline};
use seaglass_tools::bot::choose_semi_random_action;
use serde_json::Value;

/// An empty directory for one test's files.
fn scratch_dir(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("match_replay_html")
        .join(test);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Record a whole match between `sim_match`'s bots to `path`, with a note on every action.
fn record_match(path: &Path, seed: u64) {
    let player = |name: &str, deity| PlayerSetup {
        name: name.to_string(),
        deity,
        ..Default::default()
    };
    let mut game = Match::new(&MatchSetup {
        pool: PoolMode::Shared,
        shared_seed: seed,
        players: vec![
            player("P1", DeityKind::CThun),
            player("P2", DeityKind::YShaarj),
        ],
        ..Default::default()
    })
    .unwrap();
    let mut bot = Rng::new(seed);
    for _turn in 0..30 {
        game.start_turn(0);
        game.start_turn(1);
        for p in 0..2 {
            for step in 0..40 {
                let state = game.player(p);
                let forced = step == 39 && state.discover_pending.is_none();
                let action = if forced {
                    TavernAction::EndTurn
                } else {
                    choose_semi_random_action(state, &mut bot)
                };
                let legal = state.valid_actions();
                let prob = 1.0 / legal.len().max(1) as f32;
                let note = pb::Note {
                    value: Some(0.25),
                    logp: Some(prob.ln()),
                    forced,
                    candidates: legal
                        .iter()
                        .map(|a| pb::Candidate {
                            action: Some(replay::action_to_pb(a)),
                            prob,
                        })
                        .collect(),
                    ..Default::default()
                };
                match game.step_with_note(p, action, Some(note)) {
                    Ok(false) => {}
                    Ok(true) | Err(_) => break,
                }
            }
        }
        game.combat(0, 1);
        if game.players().iter().any(|s| s.health <= 0) {
            break;
        }
    }
    game.finish();
    game.save(path).unwrap();
}

fn match_replay_html(replay: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_match_replay"))
        .arg("html")
        .arg(replay)
        .args(args)
        .output()
        .expect("match_replay runs")
}

/// The replay data a page embeds.
fn embedded_data(page: &Path) -> Value {
    let page = std::fs::read_to_string(page).expect("the page was written");
    let open = r#"<script type="application/json" id="replay-data">"#;
    let start = page.find(open).expect("the page has the data element") + open.len();
    let len = page[start..]
        .find("</script>")
        .expect("the data element ends");
    serde_json::from_str(&page[start..start + len]).expect("the embedded data is JSON")
}

#[test]
fn html_writes_the_page_next_to_the_replay() {
    let dir = scratch_dir("default_output");
    let replay = dir.join("game.replay.pb");
    record_match(&replay, 3);

    let out = match_replay_html(&replay, &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let page = dir.join("game.html");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(&page.display().to_string()), "{stdout}");

    let data = embedded_data(&page);
    let timeline = Timeline::load(&replay).unwrap();
    assert_eq!(data["frames"].as_array().unwrap().len(), timeline.len());
    assert_eq!(data["source"], "game.replay.pb");
    assert_eq!(data["faithful"], true);
    assert!(data["result"].as_str().unwrap().starts_with("game over"));
    let notes = data["frames"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|frame| frame["note"].is_object())
        .count();
    assert_eq!(notes, data["actions"].as_u64().unwrap() as usize);
}

#[test]
fn html_writes_the_page_where_asked() {
    let dir = scratch_dir("chosen_output");
    let replay = dir.join("game.replay.pb");
    record_match(&replay, 4);

    let page = dir.join("pages").join("viewer.html");
    let out = match_replay_html(&replay, &["-o", page.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let data = embedded_data(&page);
    assert_eq!(
        data["frames"].as_array().unwrap().len(),
        Timeline::load(&replay).unwrap().len()
    );
    assert!(!dir.join("game.html").exists());

    let out = match_replay_html(&replay, &["-o"]);
    assert!(!out.status.success(), "-o without a file is an error");
}
