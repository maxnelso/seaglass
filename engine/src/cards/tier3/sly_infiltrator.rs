//! `Sly Infiltrator` (`BG36_330`) — Tier 3 Quilboar (`4/5`).
//! **Choose One -** Gain `2` (`4` if Golden) free **Refreshes**; or Get `3` (`6` if Golden) **Blood Gems**.

use crate::cards::tokens::{self, make_choice_option, CHOICE_SLY_GEMS, CHOICE_SLY_REFRESHES};
use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 331;
pub const NAME: &str = "Sly Infiltrator";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 5, 3)
        .with_tribe(Tribe::Quilboar)
        .on_choose_one(|state, unit, _, pool, rng| on_battlecry(state, unit, pool, rng))
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    let g = unit.is_golden;
    let opt0 = make_choice_option(CHOICE_SLY_REFRESHES, "Hug the Wall", g);
    let opt1 = make_choice_option(CHOICE_SLY_GEMS, "Nick the Gems", g);
    state.resolve_choose_one(opt0, opt1, pool, rng);
}

/// Hug the Wall: gain 2 (4 if Golden) free Refreshes.
pub fn choose_refreshes(state: &mut TavernState, chosen: &Unit, _: &mut CardPool, _: &mut Rng) {
    state.auras.free_refreshes += 2 * chosen.golden_mult() as u32;
}

/// Nick the Gems: get 3 (6 if Golden) Blood Gems.
pub fn choose_gems(state: &mut TavernState, chosen: &Unit, _: &mut CardPool, _: &mut Rng) {
    for _ in 0..3 * chosen.golden_mult() {
        state.add_to_hand(tokens::make_blood_gem());
    }
}
