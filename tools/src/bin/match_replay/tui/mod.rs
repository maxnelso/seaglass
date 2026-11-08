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
usage: match_replay tui <file> [--frame N | --turn T] [--game G] [--focus WHO] [--ascii]
                        [--dump [--step S] [--engine] [--sample win|tie|loss[:N]] [--size WxH] [--plain]]

  <file>       a seaglass replay (*.replay.pb) or a Hearthstone Power.log
  --game G     the Power.log game to play (1-based; default: the last one)
  --focus WHO  only show one player's moments (with their Duos teammate): `agent` (the RL agent
               a replay marks), a player's name or index (`p4`, `4`), or a team (`team3`, 1-based);
               `o` toggles it in the player
  --ascii      draw cards with plain ASCII (for fonts that draw some symbols two columns wide)
  --dump       print the screen at the frame (and combat step S) instead of starting the player:
               --sample shows a simulated win, tie, or loss (optional 1-based :N picks seed N),
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
        let mut app = App::new(Box::new(source), format!("{file} · {label}"), frame, glyphs);
        apply_focus(&mut app, args)?;
        return Ok(app);
    }
    let timeline = load_one(args)?;
    let frame = frame_arg(&timeline, args)?.unwrap_or(0);
    let mut app = App::new(
        Box::new(show::ReplaySource::new(timeline)),
        file,
        frame,
        glyphs,
    );
    apply_focus(&mut app, args)?;
    Ok(app)
}

/// Focus `app` on the players `--focus` names, if it names any.
fn apply_focus(app: &mut App, args: &[String]) -> Result<(), String> {
    let Some(who) = option(args, &["--focus"]) else {
        return Ok(());
    };
    let names = app.names();
    let player = if who == "agent" {
        app.source
            .agent()
            .ok_or("the replay marks no agent (`meta.controllers`)")?
    } else if let Some(team) = who.strip_prefix("team") {
        let team: usize = team
            .trim()
            .parse()
            .map_err(|_| format!("bad --focus {who:?}"))?;
        let first = (0..names.len())
            .filter(|&p| app.source.team_of(p)[0] == p)
            .nth(team.saturating_sub(1))
            .ok_or_else(|| format!("there is no team {team}"))?;
        first
    } else if let Some(p) = names.iter().position(|n| *n == who) {
        p
    } else {
        let p: usize = who.parse().map_err(|_| {
            format!(
                "--focus {who:?} names no player (players: {})",
                names.join(", ")
            )
        })?;
        if p >= names.len() {
            return Err(format!("there is no player {p}"));
        }
        p
    };
    let team = app.source.team_of(player);
    app.set_focus(Some(team));
    Ok(())
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
    if let Some(spec) = option(args, &["--sample"]) {
        if app.combat.is_none()
            && option(args, &["--frame", "-f"]).is_none()
            && option(args, &["--turn", "-t"]).is_some()
        {
            let turn = app.source.info(app.frame).turn;
            app.next_combat();
            if app.combat.is_none() || app.source.info(app.frame).turn != turn {
                return Err(format!("turn {turn} has no combat"));
            }
        }
        let (kind_str, nth) = match spec.split_once(':') {
            Some((k, n)) => {
                let n: usize = n
                    .parse()
                    .ok()
                    .filter(|&n| n >= 1)
                    .ok_or_else(|| format!("bad --sample {spec:?} (seed index must be >= 1)"))?;
                (k, n - 1)
            }
            None => (spec, 0),
        };
        let kind = combat::SampleKind::parse(kind_str).ok_or_else(|| {
            format!("bad --sample {spec:?} (expected win, tie, or loss, optionally :N)")
        })?;
        if app.combat.is_none() {
            return Err(format!("frame {} is not a combat", app.frame));
        }
        if !app.select_sample(kind, nth) {
            return Err(format!(
                "no simulated {} in this combat's sample",
                kind.label()
            ));
        }
    } else if args.iter().any(|a| a == "--engine") {
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
        KeyCode::Char('o') => app.toggle_focus(),
        KeyCode::Char('1') | KeyCode::Char('w') => {
            app.show_sample(combat::SampleKind::Win);
        }
        KeyCode::Char('2') | KeyCode::Char('t') | KeyCode::Char('d') => {
            app.show_sample(combat::SampleKind::Tie);
        }
        KeyCode::Char('3') | KeyCode::Char('L') => {
            app.show_sample(combat::SampleKind::Loss);
        }
        KeyCode::Char('0') | KeyCode::Char('r') | KeyCode::Backspace => app.clear_sample(),
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

    /// A Duos lobby played to the end by random bots.
    fn duos_lobby(seed: u64) -> Timeline {
        lobby(seed, true)
    }

    /// A lobby (Solo or Duos) played to the end by random bots.
    fn lobby(seed: u64, duos: bool) -> Timeline {
        use seaglass::{LobbyConfig, LobbyGame};
        let mut game = LobbyGame::new(LobbyConfig {
            seed,
            duos,
            heroes: false,
            ..Default::default()
        })
        .unwrap();
        let mut bot = Rng::new(seed);
        while !game.is_over() {
            game.begin_phase().unwrap();
            for _ in 0..8 {
                for p in game.active_players() {
                    let actions = game.valid_actions(p);
                    if actions.is_empty() {
                        continue;
                    }
                    let action = if actions.contains(&TavernAction::EndTurn) && bot.below(3) == 0 {
                        TavernAction::EndTurn
                    } else {
                        actions[bot.below(actions.len())]
                    };
                    game.step(p, action, None).unwrap();
                }
            }
            for p in game.active_players() {
                game.force_end(p).unwrap();
            }
            game.end_phase().unwrap();
        }
        Timeline::new(&game.game().replay()).unwrap()
    }

    #[test]
    fn up_steps_back_a_turn_at_a_time_with_a_focus() {
        let timeline = duos_lobby(3);
        let last = timeline.len() - 1;
        for team in [vec![0, 1], vec![2, 3], vec![6, 7]] {
            let mut app = App::new(
                Box::new(ReplaySource::new(timeline.clone())),
                "g.replay.pb".into(),
                last,
                Glyphs::UNICODE,
            );
            app.set_focus(Some(team.clone()));
            let first_shown = (0..app.len()).find(|&i| app.visible(i)).unwrap();
            // Every press goes to an earlier frame of an earlier (or the same, from mid-turn) turn,
            // until the first shown frame: it never gets stuck.
            let mut presses = 0;
            while app.frame != first_shown {
                let (frame, turn) = (app.frame, app.source.info(app.frame).turn);
                app.prev_turn();
                assert!(
                    app.frame < frame,
                    "team {team:?}: Up stuck at frame {frame} (turn {turn})"
                );
                assert!(app.visible(app.frame));
                presses += 1;
                assert!(presses < 2 * last, "team {team:?}: too many presses");
            }
            // Down then Up comes back to the same turn's first shown frame.
            app.next_turn();
            let down = app.frame;
            app.next_turn();
            app.prev_turn();
            assert_eq!(app.frame, down, "team {team:?}");
        }
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

    /// Check that `combat` (the playback of `frame`) re-ran faithfully and that its steps are
    /// well formed: Start first, End last, every strike followed by its settle with its attacker
    /// on the board it shows, and the events following on from step to step.
    fn check_playback(combat: &combat::CombatPlayback, what: &str) {
        assert!(combat.warning.is_none(), "{what}: {:?}", combat.warning);
        let steps = &combat.steps;
        assert_eq!(steps[0].phase, Phase::Start, "{what}");
        assert_eq!(steps.last().unwrap().phase, Phase::End, "{what}");
        for (i, step) in steps.iter().enumerate() {
            assert!(step.snapshot < combat.snapshots.len(), "{what}");
            assert!(
                step.labels.iter().all(|l| !l.is_empty()),
                "{what}: step {i} has no labels"
            );
            if step.phase == Phase::Strike {
                assert_eq!(steps[i + 1].phase, Phase::Settle, "{what}: step {i}");
                let (side, attacker) = step.attacker.expect("a strike has an attacker");
                let shown = &combat.snapshots[step.snapshot];
                let board = match side {
                    seaglass::Side::A => &shown.board_a,
                    seaglass::Side::B => &shown.board_b,
                };
                assert!(
                    board.iter().any(|u| u.id == attacker),
                    "{what}: step {i}: the attacker is not on its board"
                );
            }
            if i > 0 && step.phase != Phase::Settle {
                assert_eq!(
                    step.events.start,
                    steps[i - 1].events.end,
                    "{what}: step {i}"
                );
            }
        }
        assert_eq!(
            steps.last().unwrap().events.end,
            combat.texts.len(),
            "{what}"
        );
    }

    #[test]
    fn duos_and_ghost_combats_play_strike_by_strike() {
        let (mut duos, mut ghosts, mut step_ins) = (0, 0, 0);
        let mut duos_ghost = false;
        for (seed, is_duos) in [(1, true), (2, true), (3, true), (4, false), (5, false)] {
            let timeline = lobby(seed, is_duos);
            assert!(timeline.is_faithful(), "seed {seed}");
            let source = ReplaySource::new(timeline.clone());
            for (frame, f) in timeline.frames.iter().enumerate() {
                let ghost = match &f.kind {
                    FrameKind::DuosCombat { .. } => false,
                    FrameKind::GhostCombat { result, .. } => {
                        duos_ghost |= matches!(**result, seaglass::CombatResult::Duos(_));
                        true
                    }
                    _ => continue,
                };
                let what = format!("seed {seed} frame {frame}");
                let combat = source.combat(frame).expect("a combat to play back");
                check_playback(&combat, &what);
                if ghost {
                    ghosts += 1;
                    assert!(
                        combat.steps[0].labels[1].starts_with("ghost of "),
                        "{what}: {:?}",
                        combat.steps[0].labels
                    );
                    assert!(combat.players[1].is_empty(), "{what}");
                } else {
                    duos += 1;
                }
                for step in &combat.steps {
                    if step.phase == Phase::TagIn {
                        step_ins += 1;
                        let heading = step.heading.as_deref().unwrap_or_default();
                        assert!(heading.contains(" steps in for "), "{what}: {heading}");
                    }
                }
            }
        }
        assert!(duos > 20, "{duos} Duos combats");
        assert!(
            ghosts > 2 && duos_ghost,
            "{ghosts} ghost combats (Duos: {duos_ghost})"
        );
        assert!(step_ins > 0, "no warband stepped in");
    }

    #[test]
    fn every_duos_combat_step_draws_at_every_size() {
        let timeline = duos_lobby(6);
        let mut app = App::new(
            Box::new(ReplaySource::new(timeline)),
            "g.replay.pb".into(),
            0,
            Glyphs::UNICODE,
        );
        let mut step_in_shown = false;
        while app.next() {
            let Some(combat) = &app.combat else {
                continue;
            };
            assert!(combat.warning.is_none(), "frame {}", app.frame);
            for (w, h) in [(172, 50), (120, 36), (90, 26), (40, 12), (30, 8)] {
                let canvas = screen::draw(&app, w, h);
                assert_eq!((canvas.width(), canvas.height()), (w, h));
            }
            if combat.steps[app.step].phase == Phase::TagIn {
                let screen = screen::draw(&app, 160, 48).to_plain().join("\n");
                assert!(screen.contains(" steps in for team "), "{screen}");
                step_in_shown = true;
            }
        }
        assert!(step_in_shown, "no step-in shown");
    }

    #[test]
    fn a_duos_combat_shows_the_focused_team_at_the_bottom() {
        let timeline = duos_lobby(7);
        let frame = (0..timeline.len())
            .find(|&i| matches!(timeline.frames[i].kind, FrameKind::DuosCombat { .. }))
            .expect("a Duos combat");
        let FrameKind::DuosCombat { a, b, .. } = timeline.frames[frame].kind else {
            unreachable!()
        };
        for (unit, other) in [(a, b), (b, a)] {
            let mut app = App::new(
                Box::new(ReplaySource::new(timeline.clone())),
                "g.replay.pb".into(),
                frame,
                Glyphs::UNICODE,
            );
            app.set_focus(Some(vec![2 * unit + 1, 2 * unit]));
            assert_eq!(app.frame, frame, "a Duos combat concerns both teams");
            let screen = screen::draw(&app, 160, 48).to_plain();
            let row = |team: usize| {
                screen
                    .iter()
                    .position(|l| l.contains(&format!(" · team {} · ", team + 1)))
                    .unwrap_or_else(|| panic!("team {}: {}", team + 1, screen.join("\n")))
            };
            assert!(row(other) < row(unit), "{}", screen.join("\n"));
        }
    }

    #[test]
    fn sample_combats_play_each_available_outcome_from_either_perspective() {
        use super::combat::SampleKind;
        use seaglass::Side;

        let mut seen = [false; 3];
        let mut cycled = false;
        let mut ghost_checked = false;
        for (seed, is_duos) in [(1, false), (2, false), (3, true), (4, true)] {
            let timeline = lobby(seed, is_duos);
            let mut app = App::new(
                Box::new(ReplaySource::new(timeline.clone())),
                "g.replay.pb".into(),
                0,
                Glyphs::UNICODE,
            );
            for frame in 0..app.len() {
                if app.source.info(frame).kind != show::Kind::Combat {
                    continue;
                }
                app.go_to(frame);
                let is_ghost = matches!(timeline.frames[frame].kind, FrameKind::GhostCombat { .. });
                let players_b = app.combat.as_ref().unwrap().players[1].clone();
                let perspectives: Vec<usize> = if let Some(&p) = players_b.first() {
                    vec![0, p]
                } else {
                    vec![0]
                };
                for p in perspectives {
                    app.follow = false;
                    app.perspective = p;
                    app.clear_sample();
                    let bottom = app.bottom_side();
                    let odds = app
                        .combat
                        .as_ref()
                        .unwrap()
                        .odds
                        .as_ref()
                        .expect("combat has odds")
                        .clone();
                    assert_eq!(odds.a_wins > 0, !odds.a_win_seeds.is_empty());
                    assert_eq!(odds.draws > 0, !odds.draw_seeds.is_empty());
                    assert_eq!(odds.b_wins > 0, !odds.b_win_seeds.is_empty());

                    let (us, them) = match bottom {
                        Side::A => (&odds.side_names[0], &odds.side_names[1]),
                        Side::B => (&odds.side_names[1], &odds.side_names[0]),
                    };
                    for (idx, kind) in [SampleKind::Win, SampleKind::Tie, SampleKind::Loss]
                        .into_iter()
                        .enumerate()
                    {
                        let seeds = odds.seeds_for(bottom, kind);
                        if seeds.is_empty() {
                            assert!(!app.show_sample(kind));
                            continue;
                        }
                        seen[idx] = true;
                        ghost_checked |= is_ghost;
                        assert!(app.show_sample(kind));
                        let sample = app.shown_combat().unwrap().clone();
                        check_playback(
                            &sample,
                            &format!("seed {seed} frame {frame} {bottom:?} {kind:?}"),
                        );
                        match kind {
                            SampleKind::Win => assert!(
                                sample.result.starts_with(&format!("{us} wins · ")),
                                "expected {us} win, got {}",
                                sample.result
                            ),
                            SampleKind::Tie => assert_eq!(sample.result, "Draw"),
                            SampleKind::Loss => assert!(
                                sample.result.starts_with(&format!("{them} wins · ")),
                                "expected {them} win, got {}",
                                sample.result
                            ),
                        }
                        // Draw at multiple sizes while viewing the sample.
                        for (w, h) in [(160, 48), (90, 26), (40, 12)] {
                            let plain = screen::draw(&app, w, h).to_plain().join("\n");
                            if w >= 90 {
                                assert!(
                                    plain.contains(&format!(
                                        "SAMPLE {} 1/",
                                        kind.label().to_uppercase()
                                    )),
                                    "{plain}"
                                );
                            }
                        }
                        if seeds.len() > 1 {
                            assert!(app.show_sample(kind));
                            assert_eq!(app.sample.as_ref().unwrap().index, 1);
                            cycled = true;
                        }
                        app.clear_sample();
                        assert!(app.sample.is_none());
                    }
                }
            }
        }
        assert_eq!(seen, [true, true, true], "must exercise Win, Tie, and Loss");
        assert!(cycled, "must exercise cycling multiple sample seeds");
        assert!(ghost_checked, "must exercise ghost sample combat");

        // Flipping perspective with Tab flips SAMPLE WIN <-> SAMPLE LOSS to match bottom_side.
        let timeline = game(1);
        let frame = (0..timeline.len())
            .find(|&i| matches!(timeline.frames[i].kind, FrameKind::Combat { .. }))
            .unwrap();
        let mut app = App::new(
            Box::new(ReplaySource::new(timeline)),
            "g.replay.pb".into(),
            frame,
            Glyphs::UNICODE,
        );
        app.follow = false;
        app.perspective = 0;
        let kind = if app.show_sample(SampleKind::Win) {
            SampleKind::Win
        } else {
            assert!(app.show_sample(SampleKind::Loss));
            SampleKind::Loss
        };
        let flipped = kind.flipped();
        handle_key(&mut app, KeyCode::Tab, KeyModifiers::NONE);
        let plain = screen::draw(&app, 160, 48).to_plain().join("\n");
        assert!(
            plain.contains(&format!("SAMPLE {} 1/", flipped.label().to_uppercase())),
            "{plain}"
        );
        handle_key(&mut app, KeyCode::Char('0'), KeyModifiers::NONE);
        assert!(app.sample.is_none());
    }
}
