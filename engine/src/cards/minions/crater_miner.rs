//! `Crater Miner` (`BG31_320`) — Tier 2 Quilboar (`2/2`).
//! **Choose One -** Get 2 (`4` if Golden) **Blood Gems**; or Get a (`2` if Golden) `Gem Day`.

use crate::cards::tokens::{self, make_choice_option, CHOICE_CRATER_GEMS, CHOICE_CRATER_GEM_DAY};
use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 206;
pub const NAME: &str = "Crater Miner";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 2, 2)
        .with_tribe(Tribe::Quilboar)
        .on_choose_one(|state, unit, _, pool, rng| on_battlecry(state, unit, pool, rng))
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    let g = unit.is_golden;
    let opt0 = make_choice_option(CHOICE_CRATER_GEMS, "Take the Gems", g);
    let opt1 = make_choice_option(CHOICE_CRATER_GEM_DAY, "Mine Deeper", g);
    state.resolve_choose_one(opt0, opt1, pool, rng);
}

/// Take the Gems: get 2 (4 if Golden) Blood Gems.
pub fn choose_gems(state: &mut TavernState, chosen: &Unit, _: &mut CardPool, _: &mut Rng) {
    for _ in 0..2 * chosen.golden_mult() {
        state.add_to_hand(tokens::make_blood_gem());
    }
}

/// Mine Deeper: get a (2 if Golden) Gem Day.
pub fn choose_gem_day(state: &mut TavernState, chosen: &Unit, _: &mut CardPool, _: &mut Rng) {
    for _ in 0..chosen.golden_mult() {
        state.add_to_hand(tokens::make_gem_day());
    }
}
