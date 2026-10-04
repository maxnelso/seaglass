//! `Decoy Conjurer` (`BG36_354`) — Tier 2 Neutral (`3/4`).
//! **Activate (2):** Steal the (`2` if Golden) highest-Attack minion(s) in the Tavern.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 207;
pub const NAME: &str = "Decoy Conjurer";
pub const ACTIVATE_COST: u32 = 2;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 4, 2)
        .with_tribe(Tribe::None)
        .with_activate_cost(ACTIVATE_COST)
        .on_activate(|state, source_pos, _, _, _| on_activate(state, source_pos))
}

pub fn on_activate(state: &mut TavernState, source_pos: usize) {
    let count = if state.board[source_pos].is_golden {
        2
    } else {
        1
    };
    for _ in 0..count {
        if state.hand.len() >= 10 {
            break;
        }
        let best = state
            .shop
            .iter()
            .enumerate()
            .filter(|(_, u)| !u.is_spell)
            .max_by_key(|(idx, u)| (u.attack, std::cmp::Reverse(*idx)))
            .map(|(idx, _)| idx);
        let Some(shop_idx) = best else {
            break;
        };
        let stolen = state.shop.remove(shop_idx);
        state.add_to_hand(stolen);
    }
}
