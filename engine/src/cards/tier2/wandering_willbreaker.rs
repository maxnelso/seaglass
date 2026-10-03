//! `Wandering Willbreaker` (`BG36_100`) — Tier 2 Aberration (`1/3`).
//! When you sell this, get 2 (`4` if Golden) random Tavern spells.
//! After you cast 1 (`2` if Golden) of them, discard the other(s).

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 234;
pub const NAME: &str = "Wandering Willbreaker";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 3, 2).with_tribe(Tribe::Aberration)
}

pub fn on_sell(state: &mut TavernState, sold: &Unit, rng: &mut Rng) {
    let spell_count = if sold.is_golden { 4 } else { 2 };
    let casts_allowed = if sold.is_golden { 2 } else { 1 };
    let group_id = state.next_willbreaker_group;
    state.next_willbreaker_group += 1;

    for _ in 0..spell_count {
        if state.hand.len() >= 10 {
            break;
        }
        let mut spell = spells::draw_random_tavern_spell(state.tavern_tier, rng);
        spell.willbreaker_group = group_id;
        spell.willbreaker_remaining = casts_allowed;
        state.add_to_hand(spell);
    }
}
