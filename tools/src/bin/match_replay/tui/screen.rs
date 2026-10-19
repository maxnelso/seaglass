//! The replay player's screen, drawn into a [`Canvas`] (`match_replay tui`).
//!
//! Drawing is a pure function of the [`App`] and the terminal size, so `match_replay tui --dump`
//! prints any screen and tests can check one.

use seaglass::{Side, Unit, UnitId};
use seaglass_tools::render::{
    big_card, fit, row, row_width, slot_x, truncate, wrap, Canvas, CardSize, Color, Mark, Style,
    BIG_WIDTH,
};

use super::app::App;
use super::combat::{CombatPlayback, Phase, Step};
use super::show::Kind;

const RULE: char = '─';

/// A card on the screen, for inspection.
struct Placed {
    unit: Unit,
    x: usize,
    y: usize,
    size: CardSize,
}

/// The whole screen.
pub fn draw(app: &App, width: usize, height: usize) -> Canvas {
    let mut c = Canvas::new(width, height);
    if width < 40 || height < 12 {
        c.text(
            0,
            0,
            "The terminal is too small (40 × 12 at least).",
            Style::PLAIN,
        );
        return c;
    }
    header(app, &mut c);
    let rule = Style::fg(Color::Gray);
    for x in 0..width {
        c.put(x, 1, RULE, rule);
        c.put(x, height - 3, RULE, rule);
    }
    let body = (2, height - 3);
    let mut placed = Vec::new();
    match app.shown_combat() {
        Some(combat) => combat_body(app, combat, &mut c, body, &mut placed),
        None => tavern_body(app, &mut c, body, &mut placed),
    }
    scrubber(app, &mut c, height - 2);
    footer(app, &mut c, height - 1);
    if let Some(selected) = app.inspect {
        inspect(app, &mut c, &placed, selected, body);
    }
    if app.help {
        help(&mut c);
    }
    c
}

fn header(app: &App, c: &mut Canvas) {
    let info = app.source.info(app.frame);
    let left = format!(
        " Turn {} · frame {}/{} · {}",
        info.turn,
        app.frame,
        app.len() - 1,
        app.source.title(app.frame)
    );
    let state = if app.playing { "▶" } else { "⏸" };
    let right = format!("{state} {}×  {} ", app.speed(), app.file);
    let room = c.width().saturating_sub(right.chars().count() + 1);
    c.text(0, 0, &truncate(&left, room), Style::PLAIN.bold());
    c.text_right(c.width(), 0, &right, Style::fg(Color::Gray));
}

fn footer(app: &App, c: &mut Canvas, y: usize) {
    let keys = if app.inspect.is_some() {
        " ←/→ card  i/Esc done  ? help  q quit".to_string()
    } else if let Some(combat) = &app.combat {
        let alt = if combat.alt.is_some() {
            "  e real/engine"
        } else {
            ""
        };
        format!(" ←/→ strike  ␣ play  +/- speed  n/p frame  ↑/↓ turn  c/C combat{alt}  i inspect  ? help  q quit")
    } else {
        " ←/→ step  ␣ play  +/- speed  ↑/↓ turn  c/C combat  Tab player  f follow  i inspect  ? help  q quit".to_string()
    };
    c.text(0, y, &truncate(&keys, c.width()), Style::fg(Color::Gray));
}

/// The game as a bar, with the current frame and every combat on it.
fn scrubber(app: &App, c: &mut Canvas, y: usize) {
    let first = format!(" T{} ", app.source.info(0).turn.max(1));
    let last = format!(" T{} ", app.source.info(app.len() - 1).turn);
    let x0 = first.chars().count();
    let width = c.width().saturating_sub(x0 + last.chars().count());
    if width < 4 {
        return;
    }
    c.text(0, y, &first, Style::fg(Color::Gray));
    c.text(x0 + width, y, &last, Style::fg(Color::Gray));
    let col = |i: usize| i * (width - 1) / (app.len() - 1).max(1);
    let cursor = col(app.frame);
    for x in 0..width {
        let style = if x < cursor {
            Style::fg(Color::Cyan)
        } else {
            Style::fg(Color::Gray)
        };
        c.put(x0 + x, y, if x < cursor { '━' } else { '─' }, style);
    }
    for i in 0..app.len() {
        if app.source.info(i).kind == Kind::Combat {
            c.put(x0 + col(i), y, '┿', Style::fg(Color::Red));
        }
    }
    c.put(x0 + cursor, y, '●', Style::fg(Color::White).bold());
}

/// The Tavern of the shown player: its rows of cards, offers, and a frame log.
fn tavern_body(app: &App, c: &mut Canvas, (top, bottom): (usize, usize), placed: &mut Vec<Placed>) {
    let view = app.source.tavern(app.frame, app.shown_player());
    let width = c.width();
    let mut y = top;
    c.text(
        1,
        y,
        &truncate(&view.hero_line, width - 2),
        Style::PLAIN.bold(),
    );
    y += 1;
    let follow = if app.names().len() < 2 {
        String::new()
    } else if app.follow {
        " · following the actor".to_string()
    } else {
        " · pinned".to_string()
    };
    c.text(
        1,
        y,
        &truncate(&format!("{}{follow}", view.detail_line), width - 2),
        Style::fg(Color::Gray),
    );
    y += 1;

    // The rows, and the side panel if there is room for it.
    let side_panel = width >= row_width(7, CardSize::Small) + BIG_WIDTH + 4;
    let rows_width = if side_panel {
        width - BIG_WIDTH - 3
    } else {
        width - 2
    };
    let room = bottom - y;
    let card_rows = view
        .rows
        .iter()
        .filter(|r| !r.units.is_empty() || r.slots > 0)
        .count();
    let small_fits = room >= card_rows * (CardSize::Small.height() + 1) + 3;
    let size_for = |n: usize| {
        if small_fits {
            fit(n, rows_width)
        } else {
            CardSize::Compact
        }
    };
    let mut first_new: Option<Unit> = None;
    for r in &view.rows {
        let size = size_for(r.units.len().max(r.slots));
        if y + 2 > bottom {
            break;
        }
        c.text(
            1,
            y,
            &truncate(&r.label, rows_width),
            Style::fg(Color::Cyan).bold(),
        );
        y += 1;
        if r.units.is_empty() && r.slots == 0 {
            c.text(3, y, "(empty)", Style::fg(Color::Gray));
            y += 1;
            continue;
        }
        if y + size.height() > bottom {
            break;
        }
        if first_new.is_none() {
            first_new = r
                .units
                .iter()
                .zip(&r.fresh)
                .find(|(_, &f)| f)
                .map(|(u, _)| u.clone());
        }
        let canvas = row(&r.units, r.slots, r.slots > 0, size, &app.glyphs, |i| {
            if r.fresh.get(i).copied().unwrap_or(false) {
                Mark::Highlight
            } else {
                Mark::None
            }
        });
        c.blit(1, y, &canvas);
        for (i, unit) in r.units.iter().enumerate() {
            placed.push(Placed {
                unit: unit.clone(),
                x: 1 + slot_x(i, size),
                y,
                size,
            });
        }
        y += size.height();
    }

    for offer in &view.offers {
        if y < bottom {
            c.text(1, y, &truncate(offer, rows_width), Style::fg(Color::Yellow));
            y += 1;
        }
    }

    // The frames around this one.
    if y + 1 < bottom {
        frame_log(app, c, (y + 1, bottom), rows_width);
    }

    // The side panel: the card the action brought in.
    if side_panel && app.inspect.is_none() {
        if let Some(unit) = first_new {
            let card = big_card(&unit, &app.glyphs);
            if top + 2 + card.height() <= bottom {
                c.blit(width - BIG_WIDTH - 1, top + 2, &card);
            }
        }
    }
}

/// The titles of the frames around the current one.
fn frame_log(app: &App, c: &mut Canvas, (top, bottom): (usize, usize), width: usize) {
    let lines = bottom - top;
    let before = lines / 2;
    let first = app.frame.saturating_sub(before);
    let last = (first + lines).min(app.len());
    let first = last.saturating_sub(lines).min(first);
    for (row, i) in (first..last).enumerate() {
        let current = i == app.frame;
        let info = app.source.info(i);
        let marker = if current { "▸" } else { " " };
        let text = format!("{marker} {i:>4}  T{:<2} {}", info.turn, app.source.title(i));
        let style = match (info.kind, current) {
            (_, true) => Style::PLAIN.bold().reversed(),
            (Kind::Combat, _) => Style::fg(Color::Red),
            (Kind::TurnStart, _) => Style::fg(Color::Cyan),
            (Kind::Lobby, _) => Style::fg(Color::Gray),
            _ => Style::PLAIN,
        };
        c.text(1, top + row, &truncate(&text, width), style);
    }
}

/// A combat, strike by strike.
fn combat_body(
    app: &App,
    combat: &CombatPlayback,
    c: &mut Canvas,
    (top, bottom): (usize, usize),
    placed: &mut Vec<Placed>,
) {
    let step = &combat.steps[app.step.min(combat.steps.len() - 1)];
    let snapshot = combat.snapshots.get(step.snapshot);
    // The shown player at the bottom, their opponent on top.
    let bottom_side = if combat.players[1] == Some(app.shown_player()) {
        Side::B
    } else {
        Side::A
    };
    let empty: Vec<Unit> = Vec::new();
    let board = |side: Side| -> &[Unit] {
        match (snapshot, side) {
            (Some(s), Side::A) => &s.board_a,
            (Some(s), Side::B) => &s.board_b,
            (None, _) => &empty,
        }
    };
    let width = c.width();
    let size = fit(7, width - 2);
    let room = bottom - top;
    let size = if room >= 2 * size.height() + 10 {
        size
    } else {
        CardSize::Compact
    };
    let label = |side: Side| {
        let i = (side == Side::B) as usize;
        format!("{} · {} minions", step.labels[i], board(side).len())
    };

    let top_side = bottom_side.other();
    let mut y = top;
    if !combat.version.is_empty() {
        let other = combat.alt.is_some() || app.show_alt;
        let badge = format!(" {} ", combat.version.to_uppercase());
        let style = if combat.version == "engine" {
            Style::fg(Color::Magenta).bold().reversed()
        } else {
            Style::fg(Color::Green).bold().reversed()
        };
        c.text(1, y, &badge, style);
        if other {
            c.text(
                2 + badge.chars().count(),
                y,
                "e: switch real / engine",
                Style::fg(Color::Gray),
            );
        }
        if let Some(w) = &combat.warning {
            let room = width.saturating_sub(30 + badge.chars().count());
            c.text_right(
                width - 1,
                y,
                &truncate(w, room),
                Style::fg(Color::Yellow).bold(),
            );
        }
        y += 1;
    }
    c.text(
        1,
        y,
        &truncate(&label(top_side), width - 2),
        Style::PLAIN.bold(),
    );
    y += 1;
    let top_y = y;
    let arrow_y = top_y + size.height();
    let bottom_y = arrow_y + 3;
    let top_x = draw_side(app, c, step, board(top_side), top_side, top_y, size, placed);
    let bottom_x = draw_side(
        app,
        c,
        step,
        board(bottom_side),
        bottom_side,
        bottom_y,
        size,
        placed,
    );
    c.text(
        1,
        bottom_y + size.height(),
        &truncate(&label(bottom_side), width - 2),
        Style::PLAIN.bold(),
    );

    // The strike's arrow, from the attacker to its target.
    if let (Some((atk_side, attacker)), Some((def_side, target))) = (step.attacker, step.target) {
        let column = |side: Side, id: UnitId| {
            let x0 = if side == top_side { top_x } else { bottom_x };
            board(side)
                .iter()
                .position(|u| u.id == id)
                .map(|i| x0 + slot_x(i, size) + size.width() / 2)
        };
        if let (Some(from), Some(to)) = (column(atk_side, attacker), column(def_side, target)) {
            let up = atk_side == bottom_side;
            let amount = step
                .damage
                .iter()
                .find(|(id, _)| *id == target)
                .map(|(_, a)| format!(" -{a}"))
                .unwrap_or_default();
            arrow(c, arrow_y, from, to, up, &amount);
        }
    }

    // The caption: the phase and its events.
    let caption_y = bottom_y + size.height() + 2;
    if caption_y < bottom {
        let strikes = combat
            .steps
            .iter()
            .filter(|s| s.phase == Phase::Strike)
            .count();
        let heading = match step.phase {
            Phase::Start => "Warbands enter the combat".to_string(),
            Phase::StartOfCombat => "Start of Combat".to_string(),
            Phase::TagIn => "A warband steps in".to_string(),
            Phase::Strike => format!("Strike {}/{strikes}", step.strike + 1),
            Phase::Settle => format!("After strike {}/{strikes}", step.strike),
            Phase::End => combat.result.clone(),
        };
        let style = if step.phase == Phase::End {
            Style::fg(Color::Yellow).bold()
        } else {
            Style::fg(Color::Cyan).bold()
        };
        c.text(1, caption_y, &format!("▸ {heading}"), style);
        if combat.version.is_empty() {
            if let Some(w) = &combat.warning {
                c.text_right(
                    width - 1,
                    caption_y,
                    &format!("⚠ {w}"),
                    Style::fg(Color::Red).bold(),
                );
            }
        }
        let lines: Vec<String> = combat.texts[step.events.clone()]
            .iter()
            .flat_map(|t| wrap(t, width - 6))
            .collect();
        let room = bottom.saturating_sub(caption_y + 1);
        let skip = lines.len().saturating_sub(room);
        if skip > 0 && room > 0 {
            c.text(
                3,
                caption_y + 1,
                &format!("… {skip} more above"),
                Style::fg(Color::Gray),
            );
        }
        for (i, line) in lines.iter().skip(skip + usize::from(skip > 0)).enumerate() {
            c.text(
                3,
                caption_y + 1 + i + usize::from(skip > 0),
                line,
                Style::PLAIN,
            );
        }
    }
}

/// One side's board, centred, with the step's marks and damage. Returns its left column.
#[allow(clippy::too_many_arguments)]
fn draw_side(
    app: &App,
    c: &mut Canvas,
    step: &Step,
    units: &[Unit],
    side: Side,
    y: usize,
    size: CardSize,
    placed: &mut Vec<Placed>,
) -> usize {
    let x0 = (c.width().saturating_sub(row_width(units.len(), size))) / 2;
    let striking = step.phase == Phase::Strike;
    let canvas = row(units, 0, false, size, &app.glyphs, |i| {
        let id = units[i].id;
        if striking && step.dying.contains(&id) {
            Mark::Dying
        } else if striking && step.attacker.is_some_and(|(s, a)| s == side && a == id) {
            Mark::Highlight
        } else {
            Mark::None
        }
    });
    c.blit(x0, y, &canvas);
    for (i, unit) in units.iter().enumerate() {
        let x = x0 + slot_x(i, size);
        placed.push(Placed {
            unit: unit.clone(),
            x,
            y,
            size,
        });
        if !striking {
            continue;
        }
        // A popped shield flashes.
        if step.popped.contains(&unit.id) {
            for dy in 0..size.height() {
                for dx in [0, size.width() - 1] {
                    if let Some(cell) = c.get_mut(x + dx, y + dy) {
                        cell.style = Style::fg(Color::Yellow).bold().reversed();
                    }
                }
            }
        }
        // The damage the unit takes, on its badge row.
        if let Some((_, amount)) = step.damage.iter().find(|(id, _)| *id == unit.id) {
            let text = format!(" -{amount} ");
            let row = if size == CardSize::Small { 3 } else { 1 };
            c.text_centred(
                x + 2,
                y + row,
                size.width() - 4,
                &text,
                Style::fg(Color::Red).bold().reversed(),
            );
        }
    }
    x0
}

/// An arrow over the three rows from `y`, from column `from` to column `to`: up from the bottom
/// board (`up`) or down from the top one. `label` goes next to its head.
fn arrow(c: &mut Canvas, y: usize, from: usize, to: usize, up: bool, label: &str) {
    let style = Style::fg(Color::Yellow).bold();
    let (head_y, tail_y) = if up { (y, y + 2) } else { (y + 2, y) };
    let mid = y + 1;
    c.put(to, head_y, if up { '▲' } else { '▼' }, style);
    c.text(to + 1, head_y, label, Style::fg(Color::Red).bold());
    c.put(from, tail_y, '│', style);
    if from == to {
        c.put(from, mid, '│', style);
        return;
    }
    let (lo, hi) = (from.min(to), from.max(to));
    for x in lo..=hi {
        c.put(x, mid, '─', style);
    }
    // Corners: the path leaves the attacker's column (from below when `up`, from above otherwise),
    // runs along the middle row, and turns into the target's column.
    let (from_corner, to_corner) = match (up, to < from) {
        (true, true) => ('╮', '╰'),
        (true, false) => ('╭', '╯'),
        (false, true) => ('╯', '╭'),
        (false, false) => ('╰', '╮'),
    };
    c.put(from, mid, from_corner, style);
    c.put(to, mid, to_corner, style);
}

/// The big card of the selected card, beside it.
fn inspect(
    app: &App,
    c: &mut Canvas,
    placed: &[Placed],
    selected: usize,
    (top, bottom): (usize, usize),
) {
    if placed.is_empty() {
        return;
    }
    let card = &placed[selected % placed.len()];
    c.restyle(card.x, card.y, card.size.width(), card.size.height(), |s| {
        s.reverse = true;
    });
    let big = big_card(&card.unit, &app.glyphs);
    let x = if card.x + card.size.width() + 1 + BIG_WIDTH <= c.width() {
        card.x + card.size.width() + 1
    } else {
        card.x.saturating_sub(BIG_WIDTH + 1)
    };
    let y = card.y.min(bottom.saturating_sub(big.height())).max(top);
    let mut framed = Canvas::new(big.width(), big.height());
    framed.blit(0, 0, &big);
    c.blit(x, y, &framed);
}

const HELP: &[(&str, &str)] = &[
    ("← / →", "previous / next step (a strike, in a combat)"),
    ("space", "play / pause"),
    ("+ / -", "faster / slower"),
    ("n / p", "next / previous frame (skips a combat's strikes)"),
    ("↑ / ↓", "previous / next turn"),
    ("c / C", "next / previous combat"),
    ("Home / End", "first / last frame"),
    ("Tab", "show the next player's Tavern"),
    ("f", "follow whoever acts (on) / stay on one player"),
    (
        "e",
        "a Power.log combat: switch between the real one and the engine's",
    ),
    ("i", "inspect cards (← / → to pick one, Esc to stop)"),
    ("?", "this help"),
    ("q", "quit"),
];

fn help(c: &mut Canvas) {
    let width = 64.min(c.width() - 2);
    let height = HELP.len() + 4;
    let x = (c.width() - width) / 2;
    let y = (c.height().saturating_sub(height)) / 2;
    let mut panel = Canvas::new(width, height);
    let frame = Style::fg(Color::Cyan);
    for i in 1..width - 1 {
        panel.put(i, 0, '─', frame);
        panel.put(i, height - 1, '─', frame);
    }
    for j in 1..height - 1 {
        panel.put(0, j, '│', frame);
        panel.put(width - 1, j, '│', frame);
    }
    panel.put(0, 0, '╭', frame);
    panel.put(width - 1, 0, '╮', frame);
    panel.put(0, height - 1, '╰', frame);
    panel.put(width - 1, height - 1, '╯', frame);
    panel.text(2, 0, " Keys ", frame.bold());
    for (i, (key, what)) in HELP.iter().enumerate() {
        panel.text(3, 2 + i, key, Style::PLAIN.bold());
        panel.text(16, 2 + i, &truncate(what, width - 18), Style::PLAIN);
    }
    c.blit(x, y, &panel);
}
