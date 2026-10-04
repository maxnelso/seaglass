//! `Shoalfin Mystic` (`BG32_860`) — Tier 3 Murloc (`4/4`).
//! When you sell this, your Tavern spells give an extra `+1/+1` (`+2/+2` if Golden) this game.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 330;
pub const NAME: &str = "Shoalfin Mystic";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 4, 3)
        .with_tribe(Tribe::Murloc)
        .on_sell(|state, sold, _, _| on_sell(state, sold))
}

pub fn on_sell(state: &mut TavernState, sold: &Unit) {
    let delta = if sold.is_golden { 2 } else { 1 };
    state.auras.spell_bonus_atk += delta;
    state.auras.spell_bonus_hp += delta;
}
