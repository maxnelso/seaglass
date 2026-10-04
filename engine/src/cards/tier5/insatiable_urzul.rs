//! `Insatiable Ur'zul` (`BG21_004`) — Tier 5 Demon (`4/6`, Taunt).
//!
//! Taunt. After you play a Demon, consume a random minion in the Tavern to gain (`double` if Golden) its stats.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 527;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Insatiable Ur'zul", 4, 6, 5)
        .with_tribe(Tribe::Demon)
        .with_keyword(Keyword::Taunt)
}

pub fn after_play_minion(
    state: &mut TavernState,
    played_tribe: Tribe,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if !played_tribe.matches(Tribe::Demon) {
        return;
    }
    let urzuls: Vec<(usize, i32)> = state
        .board
        .iter()
        .enumerate()
        .filter(|&(i, u)| i != board_pos && u.card_id == ID)
        .map(|(i, u)| (i, if u.is_golden { 2 } else { 1 }))
        .collect();
    for (idx, mult) in urzuls {
        let shop_minions: Vec<usize> = state
            .shop
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.is_spell)
            .map(|(i, _)| i)
            .collect();
        if shop_minions.is_empty() {
            break;
        }
        let pick = if shop_minions.len() == 1 {
            shop_minions[0]
        } else {
            shop_minions[rng.below(shop_minions.len())]
        };
        let consumed = state.shop.remove(pick);
        pool.return_unit(&consumed);
        state.board[idx].add_stats(consumed.attack * mult, consumed.health * mult);
    }
}
