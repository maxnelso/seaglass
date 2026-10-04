//! `Snare Trapper` (`BG36_332`) — Tier 4 Quilboar (`4/4`).
//!
//! Choose One - Get a (`2` if Golden) random Quilboar; or Increase your maximum Gold by 1 (`2` if Golden).

use crate::cards::tokens::{make_choice_option, CHOICE_SNARE_MAX_GOLD, CHOICE_SNARE_QUILBOAR};
use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 453;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Snare Trapper", 4, 4, 4)
        .with_tribe(Tribe::Quilboar)
        .on_choose_one(|state, unit, _, pool, rng| on_battlecry(state, unit, pool, rng))
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    let is_golden = unit.is_golden;
    let opt0 = make_choice_option(
        CHOICE_SNARE_QUILBOAR,
        "Ensnare the Target (Get a random Quilboar)",
        is_golden,
    );
    let opt1 = make_choice_option(
        CHOICE_SNARE_MAX_GOLD,
        "Collect the Bounty (+1 Max Gold)",
        is_golden,
    );
    state.resolve_choose_one(opt0, opt1, pool, rng);
}
