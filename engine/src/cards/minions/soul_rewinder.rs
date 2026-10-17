//! `Soul Rewinder` (`BG26_174`) — Tier 2 Demon (`4/2`).
//! After your hero takes damage, rewind it and give this `+2` (`+4` if Golden) Health.
//!
//! With `Rewinder Portrait` (`trinkets::modifies`, `docs/trinkets.md` §11.8), as much Attack too.

use crate::cards::{CardFlags, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 228;
pub const NAME: &str = "Soul Rewinder";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 2, 2)
        .with_tribe(Tribe::Demon)
        .with_flags(CardFlags::REWINDS_HERO_DAMAGE)
        .on_hero_damage(on_hero_damage)
}

/// Hero damage taken in the Tavern is rewound (see [`CardFlags::REWINDS_HERO_DAMAGE`]); this
/// gains `+2` (`+4` if Golden) Health instead.
pub fn on_hero_damage(state: &mut TavernState, self_idx: usize, _amount: i32) {
    let atk = crate::trinkets::modifies(&state.auras, ID);
    let rewinder = &mut state.board[self_idx];
    let gain = 2 * rewinder.golden_mult();
    rewinder.add_stats(if atk { gain } else { 0 }, gain);
}
