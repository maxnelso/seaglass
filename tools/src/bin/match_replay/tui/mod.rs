//! `match_replay tui`: play a replay back in the terminal, with the cards drawn as character art
//! and combats played strike by strike (`engine/docs/replays.md`).
//!
//! It plays seaglass replays and games from a Hearthstone `Power.log` ([`powerlog`]).
//!
//! [`screen::draw`] draws the whole screen into a [`Canvas`] from a [`show::Source`]; ratatui
//! only puts it on the terminal. `--dump` prints a screen instead of starting the player.

mod app;
pub mod cast;
mod combat;
mod powerlog;
mod screen;
mod show;

use std::io::{self, Write};
use std::process::ExitCode;
use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color as TuiColor, Modifier, Style as TuiStyle};
use ratatui::widgets::Widget;
use seaglass_tools::render::{Canvas, Color, Glyphs, Style};

use crate::{frame_arg, load_one, option};
use app::App;
use show::Source;

pub const USAGE: &str = "\
usage: match_replay tui <file> [--frame N | --turn T] [--game G] [--ascii]
                        [--dump [--step S] [--engine] [--size WxH] [--plain]]

  <file>       a seaglass replay (*.replay.pb) or a Hearthstone Power.log
  --game G     the Power.log game to play (1-based; default: the last one)
  --ascii      draw cards with plain ASCII (for fonts that draw some symbols two columns wide)
  --dump       print the screen at the frame (and combat step S) instead of starting the player:
               --size sets the screen size (default 160x48), --plain drops the colours";

/// The app for the file named by the first argument: a seaglass replay or a `Power.log` game,
/// at the frame `--frame` / `--turn` pick.
pub(crate) fn open(args: &[String]) -> Result<App, String> {
    let path = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or_else(|| format!("missing the replay file\n\n{USAGE}"))?;
    let glyphs = if args.iter().any(|a| a == "--ascii") {
        Glyphs::ASCII
    } else {
        Glyphs::UNICODE
    };
    let file = std::path::Path::new(path)
        .file_name()
        .map_or_else(|| path.clone(), |f| f.to_string_lossy().into_owned());
    if powerlog::is_power_log(std::path::Path::new(path)) {
        let game = option(args, &["--game"])
            .map(|g| g.parse().map_err(|_| format!("bad --game {g:?}")))
            .transpose()?;
        let (source, label) = powerlog::load(std::path::Path::new(path), game)?;
        let frame = match option(args, &["--frame", "-f"]) {
            Some(f) => f.parse().map_err(|_| format!("bad --frame {f:?}"))?,
            None => match option(args, &["--turn", "-t"]) {
                Some(t) => {
                    let t: u32 = t.parse().map_err(|_| format!("bad --turn {t:?}"))?;
                    (0..source.len())
                        .find(|&i| source.info(i).turn >= t)
                        .ok_or_else(|| format!("turn {t} is not in the game"))?
                }
                None => 0,
            },
        };
        return Ok(App::new(
            Box::new(source),
            format!("{file} · {label}"),
            frame,
            glyphs,
        ));
    }
    let timeline = load_one(args)?;
    let frame = frame_arg(&timeline, args)?.unwrap_or(0);
    Ok(App::new(
        Box::new(show::ReplaySource::new(timeline)),
        file,
        frame,
        glyphs,
    ))
}

/// Run `match_replay compare`: every game of the log (or `--game G`) against the engine.
pub fn compare(args: &[String]) -> Result<ExitCode, String> {
    let path = args
        .first()
        .filter(|a| !a.starts_with('-'))
        .ok_or("usage: match_replay compare <Power.log> [--game G] [--turn T] [--full]")?;
    let path = std::path::Path::new(path);
    if !powerlog::is_power_log(path) {
        return Err(format!("{} is not a Power.log", path.display()));
    }
    let games: Vec<Option<usize>> = match option(args, &["--game"]) {
        Some(g) => vec![Some(g.parse().map_err(|_| format!("bad --game {g:?}"))?)],
        None => {
            let n = seaglass_replay::parse_power_log_file(path)?.len();
            (1..=n).map(Some).collect()
        }
    };
    for game in games {
        let turn = option(args, &["--turn", "-t"])
            .map(|t| t.parse().map_err(|_| format!("bad --turn {t:?}")))
            .transpose()?;
        let full = args.iter().any(|a| a == "--full");
        println!("{}", powerlog::compare_report(path, game, turn, full)?);
    }
    Ok(ExitCode::SUCCESS)
}

/// Run `match_replay tui`.
pub fn run(args: &[String]) -> Result<ExitCode, String> {
    let mut app = open(args)?;
    if args.iter().any(|a| a == "--dump") {
        return dump(&mut app, args);
    }
    play(&mut app).map_err(|e| format!("terminal error: {e}"))?;
    Ok(ExitCode::SUCCESS)
}

/// `--dump`: print one screen.
fn dump(app: &mut App, args: &[String]) -> Result<ExitCode, String> {
    if args.iter().any(|a| a == "--engine") {
        app.toggle_alt();
    }
    if let Some(step) = option(args, &["--step"]) {
        let step: usize = step.parse().map_err(|_| format!("bad --step {step:?}"))?;
        for _ in 0..step {
            if app.step + 1 >= app.shown_combat().map_or(1, |c| c.steps.len()) {
                break;
            }
            app.next();
        }
    }
    let (width, height) = match option(args, &["--size"]) {
        Some(size) => size
            .split_once('x')
            .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
            .ok_or_else(|| format!("bad --size {size:?} (expected WIDTHxHEIGHT)"))?,
        None => (160, 48),
    };
    let canvas = screen::draw(app, width, height);
    let lines = if args.iter().any(|a| a == "--plain") {
        canvas.to_plain()
    } else {
        canvas.to_ansi()
    };
    let mut out = io::stdout().lock();
    for line in lines {
        writeln!(out, "{}", line.trim_end()).map_err(|e| e.to_string())?;
    }
    Ok(ExitCode::SUCCESS)
}

/// The interactive player.
fn play(app: &mut App) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = (|| -> io::Result<()> {
        loop {
            terminal.draw(|frame| {
                let area = frame.area();
                let canvas = screen::draw(app, area.width as usize, area.height as usize);
                frame.render_widget(CanvasWidget(&canvas), area);
            })?;
            let wait = if app.playing {
                app.delay()
            } else {
                Duration::from_secs(60)
            };
            if event::poll(wait)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press && !handle_key(app, key.code, key.modifiers)
                    {
                        return Ok(());
                    }
                }
            } else if app.playing && !app.next() {
                app.playing = false;
            }
        }
    })();
    ratatui::restore();
    result
}

/// Apply a key. Returns `false` to quit.
fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> bool {
    if app.help {
        app.help = false;
        return !matches!(code, KeyCode::Char('q'));
    }
    if let Some(selected) = app.inspect {
        match code {
            KeyCode::Left | KeyCode::Char('h') => app.inspect = Some(selected.saturating_sub(1)),
            KeyCode::Right | KeyCode::Char('l') => app.inspect = Some(selected + 1),
            KeyCode::Esc | KeyCode::Char('i') | KeyCode::Enter => app.inspect = None,
            KeyCode::Char('q') => return false,
            _ => {}
        }
        return true;
    }
    match code {
        KeyCode::Char('q') | KeyCode::Esc => return false,
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => return false,
        KeyCode::Right | KeyCode::Char('l') => {
            app.playing = false;
            app.next();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            app.playing = false;
            app.prev();
        }
        KeyCode::Char(' ') => app.playing = !app.playing,
        KeyCode::Char('+') | KeyCode::Char('=') => app.faster(),
        KeyCode::Char('-') | KeyCode::Char('_') => app.slower(),
        KeyCode::Char('n') => app.next_frame(),
        KeyCode::Char('p') => app.prev_frame(),
        KeyCode::Down | KeyCode::Char('j') => app.next_turn(),
        KeyCode::Up | KeyCode::Char('k') => app.prev_turn(),
        KeyCode::Char('c') => app.next_combat(),
        KeyCode::Char('C') => app.prev_combat(),
        KeyCode::Home | KeyCode::Char('g') => app.go_to(0),
        KeyCode::End | KeyCode::Char('G') => app.go_to(usize::MAX),
        KeyCode::Tab => app.cycle_perspective(),
        KeyCode::Char('f') => app.toggle_follow(),
        KeyCode::Char('e') => app.toggle_alt(),
        KeyCode::Char('i') | KeyCode::Enter => {
            app.playing = false;
            app.inspect = Some(0);
        }
        KeyCode::Char('?') => app.help = true,
        _ => {}
    }
    true
}

/// A [`Canvas`] as a ratatui widget.
struct CanvasWidget<'a>(&'a Canvas);

impl Widget for CanvasWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let canvas = self.0;
        for y in 0..(area.height as usize).min(canvas.height()) {
            for x in 0..(area.width as usize).min(canvas.width()) {
                let Some(cell) = canvas.get(x, y) else {
                    continue;
                };
                if let Some(target) = buf.cell_mut((area.x + x as u16, area.y + y as u16)) {
                    target.reset();
                    target.set_char(cell.ch);
                    target.set_style(tui_style(cell.style));
                }
            }
        }
    }
}

fn tui_style(style: Style) -> TuiStyle {
    let fg = match style.fg {
        Color::Default => TuiColor::Reset,
        Color::Red => TuiColor::Red,
        Color::Green => TuiColor::Green,
        Color::Yellow => TuiColor::Yellow,
        Color::Blue => TuiColor::Blue,
        Color::Magenta => TuiColor::Magenta,
        Color::Cyan => TuiColor::Cyan,
        Color::Gray => TuiColor::DarkGray,
        Color::White => TuiColor::White,
    };
    let mut modifiers = Modifier::empty();
    if style.bold {
        modifiers |= Modifier::BOLD;
    }
    if style.dim {
        modifiers |= Modifier::DIM;
    }
    if style.reverse {
        modifiers |= Modifier::REVERSED;
    }
    TuiStyle::default().fg(fg).add_modifier(modifiers)
}

#[cfg(test)]
mod tests {
    use super::combat::Phase;
    use super::show::ReplaySource;
    use super::*;
    use seaglass::replay::FrameKind;
    use seaglass::{heroes, Match, MatchSetup, PlayerSetup, PoolMode, Rng, TavernAction, Timeline};
    use seaglass_tools::bot::choose_semi_random_action;

    /// A recorded game between `sim_match`'s bots, with heroes, trinkets and Dark Gifts.
    pub(super) fn game(seed: u64) -> Timeline {
        let offerable = heroes::offerable(false, true);
        let player = |p: u64| PlayerSetup {
            name: format!("P{}", p + 1),
            seed: seed * 10 + p,
            hero: Some(offerable[(seed * 2 + p) as usize % offerable.len()]),
            ..Default::default()
        };
        let mut game = Match::new(&MatchSetup {
            pool: PoolMode::PerPlayer,
            players: vec![player(0), player(1)],
            ..Default::default()
        })
        .unwrap();
        let mut bot = Rng::new(seed);
        for _ in 0..16 {
            game.set_pairs(&[(0, 1)]);
            game.start_turn(0);
            game.start_turn(1);
            game.deliver_gifts();
            for p in 0..2 {
                for step in 0..40 {
                    let state = game.player(p);
                    let action = if step >= 30 && state.is_legal(&TavernAction::EndTurn) {
                        TavernAction::EndTurn
                    } else {
                        choose_semi_random_action(state, &mut bot)
                    };
                    if game.step(p, action) != Ok(false) {
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
        Timeline::new(&game.replay()).unwrap()
    }

    #[test]
    fn every_frame_and_combat_step_draws_at_every_size() {
        for seed in 0..3 {
            let timeline = game(seed);
            assert!(timeline.is_faithful());
            let mut app = App::new(
                Box::new(ReplaySource::new(timeline)),
                "g.replay.pb".into(),
                0,
                Glyphs::UNICODE,
            );
            let mut combats = 0;
            loop {
                if let Some(combat) = &app.combat {
                    assert!(
                        combat.warning.is_none(),
                        "seed {seed} frame {}: the re-run diverged",
                        app.frame
                    );
                    if app.step == 0 {
                        combats += 1;
                    }
                }
                for (w, h) in [(172, 50), (120, 36), (90, 26), (40, 12), (30, 8)] {
                    let canvas = screen::draw(&app, w, h);
                    assert_eq!((canvas.width(), canvas.height()), (w, h));
                }
                if !app.next() {
                    break;
                }
            }
            assert!(combats > 3, "seed {seed}: {combats} combats");
            // The overlays draw too.
            app.help = true;
            app.inspect = Some(5);
            screen::draw(&app, 120, 36);
        }
    }

    #[test]
    fn a_combat_plays_strike_by_strike() {
        let timeline = game(4);
        let frame = (0..timeline.len())
            .find(|&i| {
                matches!(&timeline.frames[i].kind, FrameKind::Combat { result, .. }
                    if result.events.len() > 6)
            })
            .expect("a combat with a few strikes");
        let app = App::new(
            Box::new(ReplaySource::new(timeline)),
            "g.replay.pb".into(),
            frame,
            Glyphs::UNICODE,
        );
        let combat = app.combat.as_ref().unwrap();
        let phases: Vec<Phase> = combat.steps.iter().map(|s| s.phase).collect();
        assert_eq!(phases[0], Phase::Start);
        assert_eq!(*phases.last().unwrap(), Phase::End);
        // Strikes come in (strike, settle) pairs, each strike with an attacker on the board it
        // shows, and every step's events follow on from the previous step's.
        for (i, step) in combat.steps.iter().enumerate() {
            if step.phase == Phase::Strike {
                assert_eq!(combat.steps[i + 1].phase, Phase::Settle);
                let (side, attacker) = step.attacker.expect("a strike has an attacker");
                let shown = &combat.snapshots[step.snapshot];
                let board = match side {
                    seaglass::Side::A => &shown.board_a,
                    seaglass::Side::B => &shown.board_b,
                };
                assert!(board.iter().any(|u| u.id == attacker));
            }
            if i > 0 && step.phase != Phase::Settle {
                assert_eq!(step.events.start, combat.steps[i - 1].events.end);
            }
        }
        assert_eq!(combat.steps.last().unwrap().events.end, combat.texts.len());
        // The strike's screen shows its arrow.
        let mut app = app;
        while app.combat.as_ref().unwrap().steps[app.step].phase != Phase::Strike {
            app.next();
        }
        let screen = screen::draw(&app, 130, 40).to_plain().join("\n");
        assert!(screen.contains('▲') || screen.contains('▼'), "{screen}");
        assert!(screen.contains("▸ Strike 1/"), "{screen}");
    }
}
