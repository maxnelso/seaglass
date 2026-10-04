//! `Tichondrius` (`BG26_523`) — Tier 5 Demon (`4/4`).
//!
//! After your hero takes damage, give your Demons `+4/+4` (`+8/+8` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 550;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Tichondrius", 4, 4, 5)
        .with_tribe(Tribe::Demon)
        .on_hero_damage(on_hero_damage)
}

pub fn on_hero_damage(state: &mut TavernState, self_idx: usize, _amount: i32) {
    let buff = 4 * state.board[self_idx].golden_mult();
    for u in state
        .board
        .iter_mut()
        .filter(|u| u.tribe.matches(Tribe::Demon))
    {
        u.add_stats(buff, buff);
    }
}
