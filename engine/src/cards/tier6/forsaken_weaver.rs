//! `Forsaken Weaver` (`BG34_692`) — Tier 6 Undead (`3/8`).
//!
//! After you cast a Tavern spell, your Undead have `+3` (`+6` if Golden) Attack this game (wherever they are).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 611;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Forsaken Weaver", 3, 8, 6)
        .with_tribe(Tribe::Undead)
        .on_spell_cast(on_spell_cast)
}

pub fn on_spell_cast(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    state.auras.undead_bonus_attack += 3 * state.board[self_idx].golden_mult();
    state.sync_all_auras();
}
