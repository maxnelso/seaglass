//! `Orc-estra Conductor` (`BGDUO_119`) — Tier 3 Neutral (`4/4`), Duos only.
//! **Battlecry:** Give a minion `+2/+2` (Improved by each Orc-estra your team has played this
//! game) (Golden `BGDUO_119_G`, `8/8`: `+4/+4`).
//!
//! Playing it counts 1 for its player (its card counter, once per play: a repeated Battlecry
//! doesn't count again); the Battlecry gives `+2/+2` (`+4/+4` if Golden) for each Orc-estra the
//! team has played, this one included (`PlayerAuras::team_counter`, `docs/duos_tavern.md` §8.3).
//! Its target is a friendly minion, picked as `Elite Navigator`'s: the one at the play position,
//! else the one to its left; none without another minion.

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 10304;
pub const NAME: &str = "Orc-estra Conductor";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 4, 3)
        .on_play_or_magnetize(|state, _| state.auras.add_counter(ID, 1))
        .on_battlecry_target(|state, _, board_pos| pick_target(state, board_pos))
        .on_battlecry(|state, unit, _, _, _| on_battlecry(state, unit))
}

/// The friendly minion at the play position (`board_pos`), else the one to its left.
fn pick_target(state: &TavernState, board_pos: usize) -> Option<usize> {
    let len = state.board.len();
    if board_pos < len {
        Some(board_pos)
    } else {
        len.checked_sub(1)
    }
}

/// Give the target `+2/+2` (`+4/+4` if Golden) for each Orc-estra the team has played.
pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let Some(target) = cards::battlecry_target_pos(&state.board) else {
        return;
    };
    let buff = 2 * unit.golden_mult() * state.auras.team_counter(ID);
    state.board[target].add_stats(buff, buff);
}
