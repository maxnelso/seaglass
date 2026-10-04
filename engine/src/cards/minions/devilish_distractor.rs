//! `Devilish Distractor` (`BG36_762`) — Tier 5 Demon (`4/7`).
//!
//! Whenever you cast a spell on this, give minions in the Tavern `+1/+2` (`+2/+4` if Golden) this game.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 510;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Devilish Distractor", 4, 7, 5)
        .with_tribe(Tribe::Demon)
        .on_after_targeted_spell(after_targeted_spell)
}

pub fn after_targeted_spell(
    state: &mut TavernState,
    self_idx: usize,
    target_pos: usize,
    _: &mut CardPool,
    _: &mut Rng,
) {
    if self_idx != target_pos {
        return;
    }
    let mult = state.board[self_idx].golden_mult();
    let (d_atk, d_hp) = (mult, 2 * mult);
    state.auras.tavern_all_atk += d_atk;
    state.auras.tavern_all_hp += d_hp;
    for s in &mut state.shop {
        if !s.is_spell {
            s.add_stats(d_atk, d_hp);
        }
    }
}
