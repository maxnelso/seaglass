//! `Veteran Brigand` (`BG36_341`) — Tier 6 Quilboar (`8/8`).
//!
//! Choose One - This plays 3 (`6` if Golden) Blood Gems on all your minions; or cast `Blood Gem Barrage` 3 (`6` if Golden) times.

use crate::cards::tokens::{make_choice_option, CHOICE_BRIGAND_BARRAGE, CHOICE_BRIGAND_GEMS};
use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 630;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Veteran Brigand", 8, 8, 6).with_tribe(Tribe::Quilboar)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    let is_golden = unit.is_golden;
    let mut opt0 = make_choice_option(
        CHOICE_BRIGAND_GEMS,
        "Brigand's Share (Play 3 Blood Gems on all your minions)",
        is_golden,
    );
    opt0.fandral_combined = unit.fandral_combined;
    let mut opt1 = make_choice_option(
        CHOICE_BRIGAND_BARRAGE,
        "Brigand's Barrage (Cast Blood Gem Barrage 3 times)",
        is_golden,
    );
    opt1.fandral_combined = unit.fandral_combined;
    state.resolve_choose_one(opt0, opt1, pool, rng);
}
