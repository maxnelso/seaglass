//! Cards drawn as character art: the small board card, the compact card for narrow terminals,
//! and the big inspect card with rules text.
//!
//! The design (from the ASCII-art mockups):
//! - The **frame** shows what changes targeting: Taunt is a heavy line, Stealth a dashed one.
//! - The **bubble** (tall brackets just outside the frame) is Divine Shield. Its columns are always
//!   reserved, so a board never shifts when a shield pops.
//! - A **badge row** carries everything else: Windfury, Venomous, Reborn, Magnetic, Deathrattle
//!   and triggered effects (Rally, Start of Combat).
//! - Golden cards have a `★` corner and a yellow frame. Attack and Health are green when buffed
//!   above the card's base stats and Health is red when damaged.

use seaglass::cards;
use seaglass::{Tribe, Unit};

use super::canvas::{truncate, wrap, Canvas, Color, Style};
use super::text::card_text;

/// The glyphs cards are drawn with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyphs {
    /// `[taunt][stealth]`: top-left, top-right, bottom-left, bottom-right, horizontal, vertical.
    frames: [[[char; 6]; 2]; 2],
    /// Divine Shield: left top/middle/bottom, right top/middle/bottom.
    bubble: [[char; 3]; 2],
    pub golden: char,
    pub windfury: char,
    pub venomous: char,
    pub reborn: char,
    pub magnetic: char,
    pub deathrattle: char,
    pub trigger: char,
    /// What a dying card's characters fade to.
    pub fade: char,
}

impl Glyphs {
    /// Box drawing and symbols. Every glyph is one column wide outside CJK locales.
    pub const UNICODE: Glyphs = Glyphs {
        frames: [
            [
                ['╭', '╮', '╰', '╯', '─', '│'], // plain
                ['╭', '╮', '╰', '╯', '┄', '┆'], // stealth
            ],
            [
                ['┏', '┓', '┗', '┛', '━', '┃'], // taunt
                ['┏', '┓', '┗', '┛', '┅', '┇'], // taunt + stealth
            ],
        ],
        bubble: [['⎛', '⎜', '⎝'], ['⎞', '⎟', '⎠']],
        golden: '★',
        windfury: '≋',
        venomous: '☣',
        reborn: '↺',
        magnetic: '∪',
        deathrattle: '☠',
        trigger: 'ϟ',
        fade: '░',
    };

    /// Plain ASCII, for terminals whose fonts draw some symbols two columns wide.
    pub const ASCII: Glyphs = Glyphs {
        frames: [
            [
                ['+', '+', '+', '+', '-', '|'],
                ['.', '.', '\'', '\'', '.', ':'],
            ],
            [
                ['#', '#', '#', '#', '=', '#'],
                ['#', '#', '#', '#', '.', ':'],
            ],
        ],
        bubble: [['/', '(', '\\'], ['\\', ')', '/']],
        golden: '*',
        windfury: '~',
        venomous: '%',
        reborn: '@',
        magnetic: 'U',
        deathrattle: 'x',
        trigger: '!',
        fade: '.',
    };

    fn frame(&self, unit: &Unit) -> [char; 6] {
        self.frames[unit.taunt as usize][unit.stealth as usize]
    }
}

/// How big a card is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardSize {
    /// 16 × 6: two name lines, a badge row and stats. Seven fit in 118 columns.
    Small,
    /// 12 × 4: one name line, badges and stats on one row. Seven fit in 90 columns.
    Compact,
}

impl CardSize {
    /// The card's width, bubble columns included.
    pub fn width(self) -> usize {
        match self {
            CardSize::Small => 16,
            CardSize::Compact => 12,
        }
    }

    pub fn height(self) -> usize {
        match self {
            CardSize::Small => 6,
            CardSize::Compact => 4,
        }
    }

    /// The columns inside the frame.
    fn inner(self) -> usize {
        self.width() - 4
    }
}

/// The badges of a unit, in display order, with their colours.
pub fn badges(unit: &Unit, glyphs: &Glyphs) -> Vec<(char, Color)> {
    let hooks = cards::hooks(unit.card_id);
    let mut out = Vec::new();
    if unit.windfury {
        out.push((glyphs.windfury, Color::Cyan));
    }
    if unit.venomous {
        out.push((glyphs.venomous, Color::Green));
    }
    if unit.reborn {
        out.push((glyphs.reborn, Color::Cyan));
    }
    if unit.magnetic {
        out.push((glyphs.magnetic, Color::Blue));
    }
    if hooks.deathrattle.is_some() {
        out.push((glyphs.deathrattle, Color::Magenta));
    }
    if hooks.rally.is_some()
        || hooks.start_of_combat.is_some()
        || hooks.start_of_combat_action.is_some()
    {
        out.push((glyphs.trigger, Color::Yellow));
    }
    out
}

/// A tribe's short name for the bottom border.
pub fn tribe_label(tribe: Tribe) -> &'static str {
    match tribe {
        Tribe::None => "",
        Tribe::All => "All",
        other => crate::fmt::format_tribe(other),
    }
}

/// The style of a stat: green above `base`, red below `full` (damaged), plain otherwise.
fn stat_style(value: i32, base: i32, full: i32) -> Style {
    if value < full {
        Style::fg(Color::Red).bold()
    } else if value > base {
        Style::fg(Color::Green).bold()
    } else {
        Style::PLAIN.bold()
    }
}

/// The frame's style: yellow for Golden, bold for Taunt.
fn frame_style(unit: &Unit) -> Style {
    let mut style = if unit.is_golden {
        Style::fg(Color::Yellow)
    } else {
        Style::PLAIN
    };
    style.bold = unit.taunt;
    style
}

/// Draw the frame (with its corner tier and Golden star) and the bubble of a `width` × `height`
/// card. Returns the canvas and the frame's inner left column.
fn frame(unit: &Unit, size: CardSize, glyphs: &Glyphs) -> Canvas {
    let (w, h) = (size.width(), size.height());
    let mut c = Canvas::new(w, h);
    let [tl, tr, bl, br, hz, vt] = glyphs.frame(unit);
    let style = frame_style(unit);
    let (left, right) = (1, w - 2);
    c.put(left, 0, tl, style);
    c.put(right, 0, tr, style);
    c.put(left, h - 1, bl, style);
    c.put(right, h - 1, br, style);
    for x in left + 1..right {
        c.put(x, 0, hz, style);
        c.put(x, h - 1, hz, style);
    }
    for y in 1..h - 1 {
        c.put(left, y, vt, style);
        c.put(right, y, vt, style);
    }
    // The tier in the top-left corner, the Golden star in the top-right one.
    let tier = if unit.is_spell {
        unit.tavern_tier.to_string()
    } else {
        unit.tavern_tier.max(1).to_string()
    };
    c.text(left + 1, 0, &tier, style.bold());
    if unit.is_golden {
        c.put(right - 1, 0, glyphs.golden, Style::fg(Color::Yellow).bold());
    }
    // The tribe (or "Spell") on the bottom border.
    let label = if unit.is_spell {
        "Spell"
    } else {
        tribe_label(unit.tribe)
    };
    if !label.is_empty() {
        let inner = size.inner();
        let label = truncate(label, inner);
        c.text_centred(left + 1, h - 1, inner, &label, style);
    }
    if unit.divine_shield {
        let bubble = Style::fg(Color::Yellow);
        for y in 0..h {
            let k = if y == 0 {
                0
            } else if y == h - 1 {
                2
            } else {
                1
            };
            c.put(0, y, glyphs.bubble[0][k], bubble);
            c.put(w - 1, y, glyphs.bubble[1][k], bubble);
        }
    }
    c
}

/// The stats row: Attack on the left, Health on the right (or a spell's cost).
fn stats_row(c: &mut Canvas, unit: &Unit, size: CardSize, y: usize) {
    let (left, right) = (2, size.width() - 2);
    if unit.is_spell {
        let cost = if unit.costs_health {
            format!("{}♥", unit.spell_cost)
        } else {
            format!("{}g", unit.spell_cost)
        };
        c.text_centred(
            left,
            y,
            size.inner(),
            &cost,
            Style::fg(Color::Yellow).bold(),
        );
        return;
    }
    let attack = unit.attack.to_string();
    let health = unit.health.to_string();
    c.text(
        left + 1,
        y,
        &attack,
        stat_style(unit.attack, unit.base_attack, unit.attack),
    );
    c.text_right(
        right - 1,
        y,
        &health,
        stat_style(unit.health, unit.base_health, unit.max_health),
    );
}

/// The badges centred in the inner columns of row `y`, between columns `from` and `to`.
fn badge_row(c: &mut Canvas, unit: &Unit, glyphs: &Glyphs, y: usize, from: usize, to: usize) {
    let badges = badges(unit, glyphs);
    if badges.is_empty() {
        return;
    }
    let width = to - from;
    let used = (badges.len() * 2 - 1).min(width);
    let mut x = from + (width - used) / 2;
    for (ch, color) in badges {
        if x >= to {
            break;
        }
        c.put(x, y, ch, Style::fg(color).bold());
        x += 2;
    }
}

/// A board-size card ([`CardSize::Small`] or [`CardSize::Compact`]).
pub fn card(unit: &Unit, size: CardSize, glyphs: &Glyphs) -> Canvas {
    let mut c = frame(unit, size, glyphs);
    let inner = size.inner();
    let name_style = if unit.is_golden {
        Style::fg(Color::Yellow).bold()
    } else {
        Style::PLAIN
    };
    match size {
        CardSize::Small => {
            // Two name lines, bottom-aligned so short names sit just above the badges.
            let mut names = wrap(&unit.name, inner);
            names.truncate(2);
            if names.len() == 2 && wrap(&unit.name, inner).len() > 2 {
                names[1] = truncate(&format!("{}…", names[1]), inner);
            }
            let first = 3 - names.len();
            for (i, line) in names.iter().enumerate() {
                c.text_centred(2, first + i, inner, line, name_style);
            }
            badge_row(&mut c, unit, glyphs, 3, 2, 2 + inner);
            stats_row(&mut c, unit, size, 4);
        }
        CardSize::Compact => {
            c.text_centred(2, 1, inner, &truncate(&unit.name, inner), name_style);
            // Attack, badges and Health on one row.
            stats_row(&mut c, unit, size, 2);
            if !unit.is_spell {
                let attack_end = 3 + unit.attack.to_string().len();
                let health_start = size.width() - 3 - unit.health.to_string().len();
                badge_row(
                    &mut c,
                    unit,
                    glyphs,
                    2,
                    attack_end + 1,
                    health_start.max(attack_end + 1),
                );
            }
        }
    }
    c
}

/// An empty board slot of `size` (a dotted outline), for showing where a card will go.
pub fn empty_slot(size: CardSize) -> Canvas {
    let (w, h) = (size.width(), size.height());
    let mut c = Canvas::new(w, h);
    let style = Style::fg(Color::Gray).dim();
    for x in 2..w - 2 {
        c.put(x, 0, '┄', style);
        c.put(x, h - 1, '┄', style);
    }
    for y in 1..h - 1 {
        c.put(1, y, '┆', style);
        c.put(w - 2, y, '┆', style);
    }
    c
}

/// The width of the big inspect card.
pub const BIG_WIDTH: usize = 40;

/// The big inspect card: name, rules text (when known), keywords spelled out, and stats with
/// their base values.
pub fn big_card(unit: &Unit, glyphs: &Glyphs) -> Canvas {
    let inner = BIG_WIDTH - 4;
    let text_width = inner - 2;
    let mut body: Vec<(String, Style)> = Vec::new();
    body.push((String::new(), Style::PLAIN));
    if let Some(text) = card_text(unit.card_id) {
        for line in wrap(text, text_width).into_iter().take(6) {
            body.push((line, Style::PLAIN));
        }
        body.push((String::new(), Style::PLAIN));
    }
    let mut keywords: Vec<&str> = Vec::new();
    for (on, name) in [
        (unit.taunt, "Taunt"),
        (unit.divine_shield, "Divine Shield"),
        (unit.stealth, "Stealth"),
        (unit.windfury, "Windfury"),
        (unit.venomous, "Venomous"),
        (unit.reborn, "Reborn"),
        (unit.magnetic, "Magnetic"),
    ] {
        if on {
            keywords.push(name);
        }
    }
    if !keywords.is_empty() {
        for line in wrap(&keywords.join(" · "), text_width) {
            body.push((line, Style::fg(Color::Cyan)));
        }
        body.push((String::new(), Style::PLAIN));
    }
    if !unit.is_spell && (unit.attack, unit.health) != (unit.base_attack, unit.base_health) {
        body.push((
            format!("base {}/{}", unit.base_attack, unit.base_health),
            Style::fg(Color::Gray),
        ));
    }

    let height = body.len() + 4;
    let mut c = Canvas::new(BIG_WIDTH, height);
    let [tl, tr, bl, br, hz, vt] = glyphs.frame(unit);
    let style = frame_style(unit);
    let (left, right) = (1, BIG_WIDTH - 2);
    for x in left..=right {
        c.put(x, 0, hz, style);
        c.put(x, height - 1, hz, style);
    }
    for y in 0..height {
        c.put(left, y, vt, style);
        c.put(right, y, vt, style);
    }
    c.put(left, 0, tl, style);
    c.put(right, 0, tr, style);
    c.put(left, height - 1, bl, style);
    c.put(right, height - 1, br, style);
    let corner = if unit.is_spell {
        format!(" Tier {} spell ", unit.tavern_tier)
    } else {
        format!(" Tier {} ", unit.tavern_tier)
    };
    c.text(left + 2, 0, &corner, style.bold());
    if unit.is_golden {
        c.text_right(
            right - 1,
            0,
            &format!(" {} Golden ", glyphs.golden),
            Style::fg(Color::Yellow).bold(),
        );
    }
    let name = format!(" {} ", unit.name.to_uppercase());
    c.text_centred(left + 1, 1, inner + 2, &name, Style::PLAIN.bold());
    for (i, (line, line_style)) in body.iter().enumerate() {
        c.text_centred(left + 1, 2 + i, inner + 2, line, *line_style);
    }
    // The stats row, with the tribe between the stats.
    let y = height - 2;
    if unit.is_spell {
        stats_row_big_spell(&mut c, unit, y);
    } else {
        c.text(
            left + 2,
            y,
            &unit.attack.to_string(),
            stat_style(unit.attack, unit.base_attack, unit.attack),
        );
        c.text_right(
            right - 1,
            y,
            &unit.health.to_string(),
            stat_style(unit.health, unit.base_health, unit.max_health),
        );
        let tribe = tribe_label(unit.tribe);
        if !tribe.is_empty() {
            c.text_centred(
                left + 1,
                y,
                inner + 2,
                &format!("[ {tribe} ]"),
                Style::PLAIN,
            );
        }
    }
    let badges = badges(unit, glyphs);
    if !badges.is_empty() {
        let mut x = right - 1 - (badges.len() * 2 - 1);
        for (ch, color) in badges {
            c.put(x, 1, ch, Style::fg(color).bold());
            x += 2;
        }
    }
    if unit.divine_shield {
        let bubble = Style::fg(Color::Yellow);
        for y in 0..height {
            let k = if y == 0 {
                0
            } else if y == height - 1 {
                2
            } else {
                1
            };
            c.put(0, y, glyphs.bubble[0][k], bubble);
            c.put(BIG_WIDTH - 1, y, glyphs.bubble[1][k], bubble);
        }
    }
    c
}

fn stats_row_big_spell(c: &mut Canvas, unit: &Unit, y: usize) {
    let cost = if unit.costs_health {
        format!("costs {} Health", unit.spell_cost)
    } else {
        format!("costs {} Gold", unit.spell_cost)
    };
    c.text_centred(2, y, BIG_WIDTH - 4, &cost, Style::fg(Color::Yellow).bold());
}

#[cfg(test)]
mod tests {
    use super::*;
    use seaglass::cards::minions::persistent_poet;

    fn poet() -> Unit {
        seaglass::full_catalog()
            .into_iter()
            .find(|t| t.card_id == persistent_poet::ID)
            .unwrap()
            .instantiate()
    }

    #[test]
    fn the_small_card_matches_the_mockup() {
        let mut unit = poet();
        unit.windfury = true;
        unit.venomous = true;
        let lines = card(&unit, CardSize::Small, &Glyphs::UNICODE).to_plain();
        assert_eq!(
            lines,
            [
                "⎛╭4───────────╮⎞",
                "⎜│ Persistent │⎟",
                "⎜│    Poet    │⎟",
                "⎜│    ≋ ☣     │⎟",
                "⎜│ 2        3 │⎟",
                "⎝╰───Dragon───╯⎠",
            ]
        );
    }

    #[test]
    fn taunt_stealth_golden_and_the_popped_shield_change_the_outline() {
        let mut unit = poet();
        unit.divine_shield = false;
        unit.taunt = true;
        unit.is_golden = true;
        let lines = card(&unit, CardSize::Small, &Glyphs::UNICODE).to_plain();
        assert_eq!(lines[0], " ┏4━━━━━━━━━━★┓ ");
        assert_eq!(lines[5], " ┗━━━Dragon━━━┛ ");
        unit.taunt = false;
        unit.stealth = true;
        let lines = card(&unit, CardSize::Compact, &Glyphs::ASCII).to_plain();
        assert_eq!(
            lines,
            [
                " .4......*. ",
                " :Persist…: ",
                " : 2    3 : ",
                " '.Dragon.' "
            ]
        );
    }

    #[test]
    fn every_card_has_the_size_it_promises() {
        for template in seaglass::full_catalog() {
            let unit = template.instantiate();
            for size in [CardSize::Small, CardSize::Compact] {
                let c = card(&unit, size, &Glyphs::UNICODE);
                assert_eq!((c.width(), c.height()), (size.width(), size.height()));
            }
            assert_eq!(big_card(&unit, &Glyphs::UNICODE).width(), BIG_WIDTH);
        }
    }
}
