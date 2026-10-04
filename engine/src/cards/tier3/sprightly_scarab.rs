//! `Sprightly Scarab` (`BG27_084`) — Tier 3 Beast (`3/1`).
//! **Choose One -** Give a Beast `+1/+1` (`+2/+2` if Golden) and **Reborn**;
//! or `+4` (`+8` if Golden) Attack and **Windfury**.

use crate::cards::tokens::{
    make_choice_option, CHOICE_SCARAB_REBORN, CHOICE_SCARAB_WINDFURY,
};
use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 332;
pub const NAME: &str = "Sprightly Scarab";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 1, 3)
        .with_tribe(Tribe::Beast)
        .on_choose_one(|state, unit, board_pos, pool, rng| {
            on_battlecry(state, unit, board_pos, pool, rng)
        })
}

pub fn on_battlecry(
    state: &mut TavernState,
    unit: &Unit,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    // Target selection after `Sprightly Scarab` will be inserted at `board_pos`:
    // Prefer the Beast currently at `board_pos` (which shifts to `board_pos + 1`),
    // otherwise `board_pos - 1`, otherwise the first Beast on `board`, otherwise `board_pos` itself.
    let target_after_insert = if board_pos < state.board.len()
        && state.board[board_pos].tribe.matches(Tribe::Beast)
    {
        board_pos + 1
    } else if board_pos > 0
        && board_pos - 1 < state.board.len()
        && state.board[board_pos - 1].tribe.matches(Tribe::Beast)
    {
        board_pos - 1
    } else if let Some(idx) = state.board.iter().position(|u| u.tribe.matches(Tribe::Beast)) {
        if idx >= board_pos {
            idx + 1
        } else {
            idx
        }
    } else {
        return;
    };
    state.pending_choice_target = Some(target_after_insert);
    let g = unit.is_golden;
    let opt0 = make_choice_option(CHOICE_SCARAB_REBORN, "Sprightly Sprucing", g);
    let opt1 = make_choice_option(CHOICE_SCARAB_WINDFURY, "Sprightly Support", g);
    state.resolve_choose_one(opt0, opt1, pool, rng);
}
