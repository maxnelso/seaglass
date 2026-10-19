//! Terminal art for game objects: cards, rows of cards, and the [`Canvas`] they are drawn into.
//!
//! The TUI replay player (`match_replay tui`) draws with it, and so can any other tool: a
//! [`Canvas`] converts to plain or ANSI-coloured text.

pub mod board;
pub mod canvas;
pub mod card;
pub mod text;

pub use board::{fit, row, row_width, slot_x, Mark, GAP};
pub use canvas::{truncate, wrap, Canvas, Cell, Color, Style};
pub use card::{badges, big_card, card, empty_slot, CardSize, Glyphs, BIG_WIDTH};
pub use text::card_text;
