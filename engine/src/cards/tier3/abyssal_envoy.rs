//! `Abyssal Envoy` (`BG36_311`) — Tier 3 Aberration (`2/2`).
//! **Activate (0):** Discard a card in hand to get a (`2` if Golden) random Tavern spell(s).

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 301;
pub const NAME: &str = "Abyssal Envoy";
pub const ACTIVATE_COST: u32 = 0;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 2, 3)
        .with_tribe(Tribe::Aberration)
        .with_activate_cost(ACTIVATE_COST)
}

pub fn on_activate(
    state: &mut TavernState,
    source_pos: usize,
    target_pos: Option<usize>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let Some(hand_idx) = target_pos else {
        return;
    };
    if hand_idx >= state.hand.len() {
        return;
    }
    let is_golden = state.board[source_pos].is_golden;
    state.discard_hand_card(hand_idx, pool, rng);
    let count = if is_golden { 2 } else { 1 };
    for _ in 0..count {
        if state.hand.len() < 10 {
            let spell = spells::draw_random_tavern_spell(state.tavern_tier, rng);
            state.add_to_hand(spell);
        }
    }
}
