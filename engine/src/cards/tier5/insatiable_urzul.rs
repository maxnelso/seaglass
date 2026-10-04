//! `Insatiable Ur'zul` (`BG21_004`) — Tier 5 Demon (`4/6`, Taunt).
//!
//! Taunt. After you play a Demon, consume a random minion in the Tavern to gain (`double` if Golden) its stats.

use crate::cards::{CardTemplate, Played};
use crate::model::{CardId, Keyword, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 527;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Insatiable Ur'zul", 4, 6, 5)
        .with_tribe(Tribe::Demon)
        .with_keyword(Keyword::Taunt)
        .on_after_friendly_play(after_friendly_play)
}

pub fn after_friendly_play(
    state: &mut TavernState,
    self_idx: usize,
    played: &Played,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if played.magnetized || !played.tribe.matches(Tribe::Demon) {
        return;
    }
    let shop_minions: Vec<usize> = state
        .shop
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.is_spell)
        .map(|(i, _)| i)
        .collect();
    if shop_minions.is_empty() {
        return;
    }
    let pick = if shop_minions.len() == 1 {
        shop_minions[0]
    } else {
        shop_minions[rng.below(shop_minions.len())]
    };
    let consumed = state.shop.remove(pick);
    pool.return_unit(&consumed);
    let mult = if state.board[self_idx].is_golden {
        2
    } else {
        1
    };
    state.board[self_idx].add_stats(consumed.attack * mult, consumed.health * mult);
}
