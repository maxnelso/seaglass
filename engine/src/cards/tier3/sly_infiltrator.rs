//! `Sly Infiltrator` (`BG36_330`) — Tier 3 Quilboar (`4/5`).
//! **Choose One -** Gain `2` (`4` if Golden) free **Refreshes**; or Get `3` (`6` if Golden) **Blood Gems**.

use crate::cards::tokens::{make_choice_option, CHOICE_SLY_GEMS, CHOICE_SLY_REFRESHES};
use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 331;
pub const NAME: &str = "Sly Infiltrator";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 5, 3).with_tribe(Tribe::Quilboar)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    let g = unit.is_golden;
    let opt0 = make_choice_option(CHOICE_SLY_REFRESHES, "Hug the Wall", g);
    let opt1 = make_choice_option(CHOICE_SLY_GEMS, "Nick the Gems", g);
    state.resolve_choose_one(opt0, opt1, pool, rng);
}
