//! `Kalecgos, Arcane Aspect` (`BGS_041`) — Tier 5 Dragon (`4/12`).
//!
//! After you trigger a Battlecry, give your Dragons `+2/+2` (`+4/+4` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 528;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Kalecgos, Arcane Aspect", 4, 12, 5)
        .with_tribe(Tribe::Dragon)
        .on_after_friendly_battlecry(after_friendly_battlecry)
}

/// After a friendly Battlecry triggers: buff every friendly Dragon, including the played minion
/// if it is not on the board yet.
pub fn after_friendly_battlecry(
    state: &mut TavernState,
    self_idx: usize,
    played: Option<&mut Unit>,
) {
    let buff = if state.board[self_idx].is_golden {
        4
    } else {
        2
    };
    for u in &mut state.board {
        if u.tribe.matches(Tribe::Dragon) {
            u.add_stats(buff, buff);
        }
    }
    if let Some(played) = played.filter(|u| u.tribe.matches(Tribe::Dragon)) {
        played.add_stats(buff, buff);
    }
}
