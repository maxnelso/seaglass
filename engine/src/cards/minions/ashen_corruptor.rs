//! `Ashen Corruptor` (`BG32_873`) — Tier 4 Demon (`4/8`).
//!
//! After your hero takes damage, rewind it and give minions in the Tavern `+1/+2` (`+2/+4` if Golden) this turn.

use crate::cards::{CardFlags, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 402;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Ashen Corruptor", 4, 8, 4)
        .with_tribe(Tribe::Demon)
        .with_flags(CardFlags::REWINDS_HERO_DAMAGE)
        .on_hero_damage(on_hero_damage)
}

/// Hero damage taken in the Tavern is rewound (see [`CardFlags::REWINDS_HERO_DAMAGE`]); minions
/// in the Tavern get `+1/+2` (`+2/+4` if Golden) this turn instead.
pub fn on_hero_damage(state: &mut TavernState, self_idx: usize, _amount: i32) {
    let mult = state.board[self_idx].golden_mult();
    let (atk, hp) = (mult, 2 * mult);
    state.auras.tavern_turn_atk += atk;
    state.auras.tavern_turn_hp += hp;
    for s in state.shop.iter_mut().filter(|s| !s.is_spell) {
        s.add_stats(atk, hp);
    }
}
