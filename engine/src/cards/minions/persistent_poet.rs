//! `Persistent Poet` (`BG29_813`) — Tier 4 Dragon (`2/3`, Divine Shield).
//!
//! Divine Shield. Adjacent Dragons permanently keep Bonus Keywords and (`double` if Golden) stats gained in combat.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe, Unit};

pub const ID: CardId = 445;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Persistent Poet", 2, 3, 4)
        .with_tribe(Tribe::Dragon)
        .with_keyword(Keyword::DivineShield)
        .on_post_combat_neighbor_mult(post_combat_neighbor_mult)
}

/// Adjacent Dragons keep their combat gains (see [`crate::cards::keep_combat_gains`]).
pub fn post_combat_neighbor_mult(pre_board: &[Unit], self_idx: usize, neighbor_idx: usize) -> i32 {
    if pre_board[neighbor_idx].tribe.matches(Tribe::Dragon) {
        pre_board[self_idx].golden_mult()
    } else {
        0
    }
}
