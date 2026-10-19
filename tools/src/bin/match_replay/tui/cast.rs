//! `match_replay cast`: record the replay player's auto-play as an asciinema recording
//! (asciicast v2), to share a game or embed it in a page (`asciinema play game.cast`).

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use super::app::App;
use super::screen;
use crate::option;

pub const USAGE: &str = "\
usage: match_replay cast <file> [-o out.cast] [--frame N | --turn T] [--game G] [--frames K]
                         [--size WxH] [--speed X] [--ascii]

  Records the player's auto-play from the frame (default: the start) for K frames (default: to
  the end), combats strike by strike, at the player's pace times --speed (default 1).
  --size sets the screen size (default 160x48).";

/// Run `match_replay cast`.
pub fn run(args: &[String]) -> Result<ExitCode, String> {
    let mut app = super::open(args)?;
    let start = app.frame;
    let input = PathBuf::from(&args[0]);
    let file = input
        .file_name()
        .map_or_else(|| args[0].clone(), |f| f.to_string_lossy().into_owned());
    let out = option(args, &["-o", "--output"]).map_or_else(
        || {
            let stem = file.strip_suffix(".replay.pb").unwrap_or(&file);
            input.with_file_name(format!("{stem}.cast"))
        },
        PathBuf::from,
    );
    let (width, height) = match option(args, &["--size"]) {
        Some(size) => size
            .split_once('x')
            .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
            .ok_or_else(|| format!("bad --size {size:?} (expected WIDTHxHEIGHT)"))?,
        None => (160, 48),
    };
    let speed: f64 = match option(args, &["--speed"]) {
        Some(s) => s
            .parse()
            .ok()
            .filter(|s: &f64| *s > 0.0)
            .ok_or_else(|| format!("bad --speed {s:?}"))?,
        None => 1.0,
    };
    let frames: Option<usize> = option(args, &["--frames"])
        .map(|k| k.parse().map_err(|_| format!("bad --frames {k:?}")))
        .transpose()?;

    app.playing = true;
    let end = frames.map_or(usize::MAX, |k| start + k);
    let cast = record(&mut app, width, height, speed, end, &file);
    fs::write(&out, cast).map_err(|e| format!("{}: {e}", out.display()))?;
    eprintln!("recorded {} to {}", file, out.display());
    Ok(ExitCode::SUCCESS)
}

/// The asciicast of auto-play from the app's frame until frame `end` (or the replay's end).
/// After the first screen, each event only redraws the rows that changed.
fn record(
    app: &mut App,
    width: usize,
    height: usize,
    speed: f64,
    end: usize,
    title: &str,
) -> String {
    let header = serde_json::json!({
        "version": 2,
        "width": width,
        "height": height,
        "title": format!("seaglass replay: {title}"),
        "env": { "TERM": "xterm-256color" },
    });
    let mut out = format!("{header}\n");
    let mut time = 0.0f64;
    let mut previous: Vec<String> = Vec::new();
    loop {
        let lines = screen::draw(app, width, height).to_ansi();
        let mut data = String::new();
        if previous.is_empty() {
            data.push_str("\x1b[?25l\x1b[2J");
        }
        for (row, line) in lines.iter().enumerate() {
            if previous.get(row) != Some(line) {
                data.push_str(&format!("\x1b[{};1H{line}", row + 1));
            }
        }
        if !data.is_empty() {
            out.push_str(&serde_json::json!([round(time), "o", data]).to_string());
            out.push('\n');
        }
        previous = lines;
        time += app.delay().as_secs_f64() / speed;
        if app.frame + 1 >= end && app.step + 1 >= app.combat.as_ref().map_or(1, |c| c.steps.len())
        {
            break;
        }
        if !app.next() {
            break;
        }
    }
    // Hold the last screen, then give the cursor back.
    out.push_str(&serde_json::json!([round(time + 2.0), "o", "\x1b[?25h"]).to_string());
    out.push('\n');
    out
}

fn round(t: f64) -> f64 {
    (t * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cast_is_asciicast_v2() {
        let timeline = super::super::tests::game(1);
        let mut app = App::new(
            Box::new(super::super::show::ReplaySource::new(timeline)),
            "g.replay.pb".into(),
            0,
            seaglass_tools::render::Glyphs::UNICODE,
        );
        let cast = record(&mut app, 120, 36, 2.0, 60, "g");
        let mut lines = cast.lines();
        let header: serde_json::Value = serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(header["version"], 2);
        assert_eq!(header["width"], 120);
        let mut last = -1.0;
        let mut events = 0;
        for line in lines {
            let event: serde_json::Value = serde_json::from_str(line).unwrap();
            let t = event[0].as_f64().unwrap();
            assert!(t >= last, "time goes forward");
            assert_eq!(event[1], "o");
            last = t;
            events += 1;
        }
        assert!(events > 30, "{events} events");
        assert!(app.frame >= 59, "it played up to the end frame");
    }
}
