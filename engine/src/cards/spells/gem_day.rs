//! `Gem Day` (`BG31_893`) — Tier 2 token spell (`1` Gold).
//!
//! Choose One - Your Blood Gems give an extra +1 Attack this game; or +1 Health.

use super::spell;
use crate::cards::tokens::{make_choice_option, CHOICE_GEM_DAY_ATK, CHOICE_GEM_DAY_HP};
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Gem Day`: Choose One - your Blood Gems give an extra +1 Attack this game; or +1 Health.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    let opt0 = make_choice_option(CHOICE_GEM_DAY_ATK, "Gem Day (+1 Attack)", false);
    let opt1 = make_choice_option(CHOICE_GEM_DAY_HP, "Gem Day (+1 Health)", false);
    state.resolve_choose_one(opt0, opt1, pool, rng);
}

/// `Gem Day`: your Blood Gems give an extra +1 Attack this game.
pub fn choose_atk(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    state.auras.blood_gem_bonus_atk += 1;
}

/// `Gem Day`: your Blood Gems give an extra +1 Health this game.
pub fn choose_hp(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    state.auras.blood_gem_bonus_hp += 1;
}
