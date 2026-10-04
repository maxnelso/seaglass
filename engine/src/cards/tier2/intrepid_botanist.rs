//! `Intrepid Botanist` (`BG32_237`) — Tier 2 Neutral (`3/4`).
//! **Choose One -** Your Tavern spells give an extra `+1` (`+2` if Golden) Attack this game; or `+1` (`+2` if Golden) Health.

use crate::cards::tokens::{make_choice_option, CHOICE_BOTANIST_ATK, CHOICE_BOTANIST_HP};
use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 215;
pub const NAME: &str = "Intrepid Botanist";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 4, 2)
        .with_tribe(Tribe::None)
        .on_choose_one(|state, unit, _, pool, rng| on_battlecry(state, unit, pool, rng))
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    let g = unit.is_golden;
    let opt0 = make_choice_option(CHOICE_BOTANIST_ATK, "Pristine Lilies", g);
    let opt1 = make_choice_option(CHOICE_BOTANIST_HP, "Giant Dewdrop", g);
    state.resolve_choose_one(opt0, opt1, pool, rng);
}
