//! `Fearless Foodie` (`BG30_123`) — Tier 3 Quilboar (`2/4`).
//! **Choose One -** Your **Blood Gems** give an extra `+1/+1` (`+2/+2` if Golden) this game;
//! or Get `4` (`8` if Golden) **Blood Gems**.

use crate::cards::tokens::{
    make_choice_option, CHOICE_FOODIE_BUFF_GEMS, CHOICE_FOODIE_GET_GEMS,
};
use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 314;
pub const NAME: &str = "Fearless Foodie";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 4, 3).with_tribe(Tribe::Quilboar)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    let g = unit.is_golden;
    let opt0 = make_choice_option(CHOICE_FOODIE_BUFF_GEMS, "Culinary Creation", g);
    let opt1 = make_choice_option(CHOICE_FOODIE_GET_GEMS, "Side of Gems", g);
    state.resolve_choose_one(opt0, opt1, pool, rng);
}
