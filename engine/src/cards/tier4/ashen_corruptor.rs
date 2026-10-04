//! `Ashen Corruptor` (`BG32_873`) — Tier 4 Demon (`6/6`).
//!
//! After your hero takes damage, rewind it and give minions in the Tavern `+2/+2` (`+4/+4` if Golden) this turn.

use crate::cards::{CardFlags, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 402;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Ashen Corruptor", 6, 6, 4)
        .with_tribe(Tribe::Demon)
        .with_flags(CardFlags::REWINDS_HERO_DAMAGE)
        .on_hero_damage(on_hero_damage)
}

/// Hero damage taken in the Tavern is rewound (see [`CardFlags::REWINDS_HERO_DAMAGE`]); minions
/// in the Tavern get `+2/+2` (`+4/+4` if Golden) this turn instead.
pub fn on_hero_damage(state: &mut TavernState, self_idx: usize, _amount: i32) {
    let buff = 2 * state.board[self_idx].golden_mult();
    state.auras.ashen_corruptor_turn_buff += buff;
    for s in state.shop.iter_mut().filter(|s| !s.is_spell) {
        s.add_stats(buff, buff);
    }
}
