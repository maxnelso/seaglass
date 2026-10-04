//! `Ichoron the Protector` (`BG31_812`) — Tier 4 Elemental (`3/1`, Divine Shield).
//!
//! Divine Shield. Whenever you play an Elemental, give it Divine Shield until next turn (`permanently` if Golden).

use crate::cards::{CardTemplate, Played};
use crate::model::{CardId, Keyword, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 433;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Ichoron the Protector", 3, 1, 4)
        .with_tribe(Tribe::Elemental)
        .with_keyword(Keyword::DivineShield)
        .on_after_friendly_play(after_friendly_play)
}

pub fn after_friendly_play(
    state: &mut TavernState,
    self_idx: usize,
    played: &Played,
    _: &mut CardPool,
    _: &mut Rng,
) {
    if played.magnetized
        || !played.tribe.matches(Tribe::Elemental)
        || played.board_pos >= state.board.len()
    {
        return;
    }
    let permanent = state.board[self_idx].is_golden;
    let target = &mut state.board[played.board_pos];
    if permanent {
        target.divine_shield = true;
        target.temp_divine_shield = false;
    } else if !target.divine_shield {
        target.divine_shield = true;
        target.temp_divine_shield = true;
    }
}
