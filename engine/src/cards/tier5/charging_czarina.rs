//! `Charging Czarina` (`BG28_741`) — Tier 5 Mech (`4/1`, Divine Shield).
//!
//! Divine Shield. Whenever you cast a Tavern spell, give your minions with Divine Shield `+4` (`+8` if Golden) Attack.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 506;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Charging Czarina", 4, 1, 5)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::DivineShield)
        .on_spell_cast(on_spell_cast)
}

pub fn on_spell_cast(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let atk = 4 * state.board[self_idx].golden_mult();
    for u in state.board.iter_mut().filter(|u| u.divine_shield) {
        u.add_stats(atk, 0);
    }
}
