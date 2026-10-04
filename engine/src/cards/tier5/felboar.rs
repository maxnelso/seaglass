//! `Felboar` (`BG28_633`) — Tier 5 Demon / Quilboar (`2/6`).
//!
//! After you cast 3 spells, consume a minion in the Tavern to gain (`double` if Golden) its stats.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 519;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Felboar", 2, 6, 5).with_tribe(Tribe::DemonQuilboar)
}

pub fn after_cast_any_spell(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let mut triggers: Vec<(usize, i32)> = Vec::new();
    for (idx, u) in state.board.iter_mut().enumerate() {
        if u.card_id == ID {
            u.felboar_spell_progress += 1;
            if u.felboar_spell_progress >= 3 {
                u.felboar_spell_progress -= 3;
                let mult = if u.is_golden { 2 } else { 1 };
                triggers.push((idx, mult));
            }
        }
    }
    for (board_idx, mult) in triggers {
        if board_idx >= state.board.len() {
            continue;
        }
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
        state.board[board_idx].add_stats(consumed.attack * mult, consumed.health * mult);
    }
}
