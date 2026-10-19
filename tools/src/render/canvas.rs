//! A grid of styled characters ([`Canvas`]) that the renderers draw into.
//!
//! Every cell holds one character that takes one terminal column, so a canvas maps 1:1 onto a
//! terminal region. It knows nothing about any terminal library: the TUI converts it into its own
//! buffer, and [`Canvas::to_plain`] / [`Canvas::to_ansi`] turn it into text.

/// A foreground colour (the terminal's own palette, so themes apply).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Color {
    #[default]
    Default,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Gray,
    White,
}

impl Color {
    /// The ANSI SGR foreground code.
    fn sgr(self) -> Option<u8> {
        Some(match self {
            Color::Default => return None,
            Color::Red => 31,
            Color::Green => 32,
            Color::Yellow => 33,
            Color::Blue => 34,
            Color::Magenta => 35,
            Color::Cyan => 36,
            Color::Gray => 90,
            Color::White => 97,
        })
    }
}

/// How a cell is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Style {
    pub fg: Color,
    pub bold: bool,
    pub dim: bool,
    pub reverse: bool,
}

impl Style {
    pub const PLAIN: Style = Style {
        fg: Color::Default,
        bold: false,
        dim: false,
        reverse: false,
    };

    pub fn fg(fg: Color) -> Self {
        Self { fg, ..Self::PLAIN }
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    pub fn reversed(mut self) -> Self {
        self.reverse = true;
        self
    }

    /// The ANSI SGR sequence that switches to this style from the plain one.
    fn sgr(self) -> String {
        let mut codes: Vec<String> = Vec::new();
        if self.bold {
            codes.push("1".into());
        }
        if self.dim {
            codes.push("2".into());
        }
        if self.reverse {
            codes.push("7".into());
        }
        if let Some(fg) = self.fg.sgr() {
            codes.push(fg.to_string());
        }
        if codes.is_empty() {
            String::new()
        } else {
            format!("\x1b[{}m", codes.join(";"))
        }
    }
}

/// One terminal column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub style: Style,
}

impl Cell {
    pub const BLANK: Cell = Cell {
        ch: ' ',
        style: Style::PLAIN,
    };
}

/// A `width` × `height` grid of [`Cell`]s, blank to start with. Drawing outside it is clipped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canvas {
    width: usize,
    height: usize,
    cells: Vec<Cell>,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::BLANK; width * height],
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn get(&self, x: usize, y: usize) -> Option<&Cell> {
        (x < self.width && y < self.height).then(|| &self.cells[y * self.width + x])
    }

    pub fn get_mut(&mut self, x: usize, y: usize) -> Option<&mut Cell> {
        (x < self.width && y < self.height).then(|| &mut self.cells[y * self.width + x])
    }

    /// Draw one character.
    pub fn put(&mut self, x: usize, y: usize, ch: char, style: Style) {
        if let Some(cell) = self.get_mut(x, y) {
            *cell = Cell { ch, style };
        }
    }

    /// Draw `text` from `(x, y)` rightwards, clipped to the canvas. Returns the columns drawn.
    pub fn text(&mut self, x: usize, y: usize, text: &str, style: Style) -> usize {
        let mut n = 0;
        for (i, ch) in text.chars().enumerate() {
            if x + i >= self.width {
                break;
            }
            self.put(x + i, y, ch, style);
            n += 1;
        }
        n
    }

    /// Draw `text` centred in the `width` columns from `x`.
    pub fn text_centred(&mut self, x: usize, y: usize, width: usize, text: &str, style: Style) {
        let len = text.chars().count().min(width);
        self.text(x + (width - len) / 2, y, &truncate(text, width), style);
    }

    /// Draw `text` so that it ends at column `right` (exclusive).
    pub fn text_right(&mut self, right: usize, y: usize, text: &str, style: Style) {
        let len = text.chars().count();
        self.text(right.saturating_sub(len), y, text, style);
    }

    /// Copy `other` onto this canvas with its top-left corner at `(x, y)`.
    pub fn blit(&mut self, x: usize, y: usize, other: &Canvas) {
        for oy in 0..other.height {
            for ox in 0..other.width {
                let cell = other.cells[oy * other.width + ox];
                if let Some(target) = self.get_mut(x + ox, y + oy) {
                    *target = cell;
                }
            }
        }
    }

    /// Change the style of every cell in a rectangle.
    pub fn restyle(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        f: impl Fn(&mut Style),
    ) {
        for cy in y..(y + height).min(self.height) {
            for cx in x..(x + width).min(self.width) {
                f(&mut self.cells[cy * self.width + cx].style);
            }
        }
    }

    /// Change the style of every cell.
    pub fn restyle_all(&mut self, f: impl Fn(&mut Style)) {
        self.cells.iter_mut().for_each(|c| f(&mut c.style));
    }

    /// Change every non-blank character (dying cards fade to `░`).
    pub fn map_chars(&mut self, f: impl Fn(char) -> char) {
        for cell in &mut self.cells {
            if cell.ch != ' ' {
                cell.ch = f(cell.ch);
            }
        }
    }

    /// The canvas as plain text, one line per row (trailing spaces kept).
    pub fn to_plain(&self) -> Vec<String> {
        self.cells
            .chunks(self.width.max(1))
            .take(self.height)
            .map(|row| row.iter().map(|c| c.ch).collect())
            .collect()
    }

    /// The canvas as text with ANSI colours, one line per row, each ending in a style reset.
    pub fn to_ansi(&self) -> Vec<String> {
        self.cells
            .chunks(self.width.max(1))
            .take(self.height)
            .map(|row| {
                let mut line = String::new();
                let mut current = Style::PLAIN;
                for cell in row {
                    if cell.style != current {
                        line.push_str("\x1b[0m");
                        line.push_str(&cell.style.sgr());
                        current = cell.style;
                    }
                    line.push(cell.ch);
                }
                if current != Style::PLAIN {
                    line.push_str("\x1b[0m");
                }
                line
            })
            .collect()
    }
}

/// `text` cut to at most `width` characters, ending in `…` if it was cut.
pub fn truncate(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if len <= width {
        text.to_string()
    } else if width == 0 {
        String::new()
    } else {
        text.chars().take(width - 1).chain(['…']).collect()
    }
}

/// `text` word-wrapped into lines of at most `width` characters (over-long words are cut).
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let word = truncate(word, width);
        if current.is_empty() {
            current = word;
        } else if current.chars().count() + 1 + word.chars().count() <= width {
            current.push(' ');
            current.push_str(&word);
        } else {
            lines.push(std::mem::take(&mut current));
            current = word;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawing_is_clipped_and_blits_overlay() {
        let mut canvas = Canvas::new(5, 2);
        assert_eq!(canvas.text(3, 0, "abcdef", Style::PLAIN), 2);
        let mut small = Canvas::new(2, 1);
        small.text(0, 0, "xy", Style::fg(Color::Red));
        canvas.blit(0, 1, &small);
        assert_eq!(canvas.to_plain(), vec!["   ab", "xy   "]);
        assert_eq!(canvas.to_ansi()[1], "\x1b[0m\x1b[31mxy\x1b[0m   ");
    }

    #[test]
    fn text_wraps_and_truncates() {
        assert_eq!(
            wrap("Divine Shield. Adjacent Dragons", 14),
            ["Divine Shield.", "Adjacent", "Dragons"]
        );
        assert_eq!(truncate("Persistent", 6), "Persi…");
        assert_eq!(truncate("Poet", 6), "Poet");
    }
}
