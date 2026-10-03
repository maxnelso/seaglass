//! `Intrepid Botanist` (`BG32_237`) — Tier 2 Neutral (`3/4`).
//! **Choose One -** Your Tavern spells give an extra `+1` (`+2` if Golden) Attack this game; or `+1` (`+2` if Golden) Health.

use crate::cards::tokens::{
    make_choice_option, CHOICE_BOTANIST_ATK, CHOICE_BOTANIST_HP,
};
use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 215;
pub const NAME: &str = "Intrepid Botanist";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 4, 2).with_tribe(Tribe::None)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let g = unit.is_golden;
    let opts = vec![
        make_choice_option(CHOICE_BOTANIST_ATK, "Pristine Lilies", g),
        make_choice_option(CHOICE_BOTANIST_HP, "Giant Dewdrop", g),
    ];
    state.push_discover(opts);
}
