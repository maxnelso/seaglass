//! `Boundless Potential` — Tier 4 Tavern spell (`3` Gold).
//!
//! Choose One - Discover a minion of your Tier; or a Tavern spell of your Tier.

use super::{draw_discover_tavern_spells_exact_tier, spell};
use crate::cards::tokens::{make_choice_option, CHOICE_BOUNDLESS_MINION, CHOICE_BOUNDLESS_SPELL};
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Boundless Potential`: Choose One - Discover a minion of your Tier; or a Tavern spell of your
/// Tier.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    let opt0 = make_choice_option(
        CHOICE_BOUNDLESS_MINION,
        "Way of the Warrior (Discover a minion of your Tier)",
        false,
    );
    let opt1 = make_choice_option(
        CHOICE_BOUNDLESS_SPELL,
        "Way of the Mage (Discover a Tavern spell of your Tier)",
        false,
    );
    state.resolve_choose_one(opt0, opt1, pool, rng);
}

/// `Boundless Potential` (Way of the Warrior): Discover a minion of your Tier.
pub fn choose_minion(state: &mut TavernState, _: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    let mut opts = pool.draw_discover_options(state.tavern_tier, 3, rng);
    for opt in &mut opts {
        state.apply_global_unit_auras(opt);
    }
    if !opts.is_empty() {
        state.push_discover(opts);
    }
}

/// `Boundless Potential` (Way of the Mage): Discover a Tavern spell of your Tier.
pub fn choose_spell(state: &mut TavernState, _: &Unit, _: &mut CardPool, rng: &mut Rng) {
    let opts = draw_discover_tavern_spells_exact_tier(state.tavern_tier, 3, rng);
    if !opts.is_empty() {
        state.push_discover(opts);
    }
}
