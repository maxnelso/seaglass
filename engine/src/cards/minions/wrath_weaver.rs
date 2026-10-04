//! `Wrath Weaver` (`BGS_004`) — Tier 1 Demon (`1/3`).
//! After you play a Demon, deal 1 damage to your hero and gain `+2/+2` (twice if Golden).

use crate::cards::{CardTemplate, Played};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 105;
pub const NAME: &str = "Wrath Weaver";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 3, 1)
        .with_tribe(Tribe::Demon)
        .on_after_friendly_play(after_friendly_play)
}

pub fn after_friendly_play(
    state: &mut TavernState,
    self_idx: usize,
    played: &Played,
    _: &mut CardPool,
    _: &mut Rng,
) {
    if played.magnetized || !played.tribe.matches(Tribe::Demon) {
        return;
    }
    let triggers = if state.board[self_idx].is_golden {
        2
    } else {
        1
    };
    for _ in 0..triggers {
        state.deal_hero_damage(1);
        state.board[self_idx].add_stats(2, 2);
    }
}
