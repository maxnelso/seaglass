//! A row of cards: a board, a shop or a hand.

use seaglass::Unit;

use super::canvas::{Canvas, Color, Style};
use super::card::{card, empty_slot, CardSize, Glyphs};

/// The columns between two cards of a row.
pub const GAP: usize = 1;

/// How one card of a row is decorated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mark {
    #[default]
    None,
    /// Drawn in reverse video (the attacker, the card an action uses).
    Highlight,
    /// Faded to `░` and dimmed (a minion dying).
    Dying,
    /// Dimmed (a frozen shop card, a card not yet in play).
    Dim,
}

/// The left column of card `i` of a row.
pub fn slot_x(i: usize, size: CardSize) -> usize {
    i * (size.width() + GAP)
}

/// The width of a row of `n` cards.
pub fn row_width(n: usize, size: CardSize) -> usize {
    if n == 0 {
        0
    } else {
        n * size.width() + (n - 1) * GAP
    }
}

/// The widest card size that fits `n` cards in `width` columns (compact if none does).
pub fn fit(n: usize, width: usize) -> CardSize {
    if row_width(n, CardSize::Small) <= width {
        CardSize::Small
    } else {
        CardSize::Compact
    }
}

/// A row of `units`, at least `slots` wide (missing cards are drawn as empty slots when
/// `show_empty`), each decorated by `mark(i)`.
pub fn row(
    units: &[Unit],
    slots: usize,
    show_empty: bool,
    size: CardSize,
    glyphs: &Glyphs,
    mark: impl Fn(usize) -> Mark,
) -> Canvas {
    let n = units.len().max(slots);
    let mut c = Canvas::new(row_width(n, size).max(1), size.height());
    for i in 0..n {
        let mut drawn = match units.get(i) {
            Some(unit) => card(unit, size, glyphs),
            None if show_empty => empty_slot(size),
            None => continue,
        };
        match mark(i) {
            Mark::None => {}
            Mark::Highlight => drawn.restyle_all(|s| s.reverse = true),
            Mark::Dying => {
                drawn.map_chars(|_| glyphs.fade);
                drawn.restyle_all(|s| {
                    *s = Style::fg(Color::Gray).dim();
                });
            }
            Mark::Dim => drawn.restyle_all(|s| s.dim = true),
        }
        c.blit(slot_x(i, size), 0, &drawn);
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seven_small_cards_fit_118_columns() {
        assert_eq!(row_width(7, CardSize::Small), 118);
        assert_eq!(row_width(7, CardSize::Compact), 90);
        assert_eq!(fit(7, 120), CardSize::Small);
        assert_eq!(fit(7, 100), CardSize::Compact);
        assert_eq!(slot_x(2, CardSize::Small), 34);
    }

    #[test]
    fn a_row_marks_its_cards() {
        let units: Vec<Unit> = seaglass::tier1_catalog()
            .into_iter()
            .take(2)
            .map(|t| t.instantiate())
            .collect();
        let c = row(&units, 3, true, CardSize::Compact, &Glyphs::UNICODE, |i| {
            if i == 1 {
                Mark::Dying
            } else {
                Mark::None
            }
        });
        assert_eq!(c.width(), row_width(3, CardSize::Compact));
        let lines = c.to_plain();
        assert!(lines[1][..].contains('░'));
        assert!(lines[0].ends_with("┄┄┄┄┄┄┄┄  "), "{:?}", lines[0]);
    }
}
