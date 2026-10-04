//! `Unmasked Identity` — Tier 5 Tavern spell (`3` Gold).
//!
//! Discover a Hero Power.

use super::spell;
use crate::cards::tokens::{make_choice_option, CHOICE_HP_1, CHOICE_HP_2, CHOICE_HP_3};
use crate::cards::{CardFlags, CardHooks};
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast).with_flags(CardFlags::NOT_IN_POOL)
}

/// `Unmasked Identity`: Discover a Hero Power.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    let opts = vec![
        make_choice_option(CHOICE_HP_1, "Hero Power Option 1", false),
        make_choice_option(CHOICE_HP_2, "Hero Power Option 2", false),
        make_choice_option(CHOICE_HP_3, "Hero Power Option 3", false),
    ];
    state.push_discover(opts);
}

/// `Unmasked Identity`: take the chosen Hero Power.
pub fn choose_hero_power(state: &mut TavernState, chosen: &Unit, _: &mut CardPool, _: &mut Rng) {
    state.auras.hero_power_id = match chosen.card_id {
        CHOICE_HP_1 => 1,
        CHOICE_HP_2 => 2,
        CHOICE_HP_3 => 3,
        _ => return,
    };
}
