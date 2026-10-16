//! `Master Nguyen` (`BG20_HERO_202`) — Armor 10 (Duos 8).
//!
//! Hero Power: `Power of the Storm` (`BG20_HERO_202p`), 0 Gold: At the start of every turn, choose from 2 new Hero Powers.
//!
//! Passive. At the start of every turn, its `hero_base_turn_reset` hook (which runs for the hero's
//! own Hero Power whatever power replaced it) puts `Power of the Storm` back
//! (`super::replace_power`: the outgoing power cleans up through its `hero_power_removed` hook),
//! and `turn_start` offers a Discover of 2 new Hero Powers for that turn.

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 71908;
pub const POWER: CardId = 71909;

/// Options offered each turn.
const OPTIONS: usize = 2;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_base_turn_reset(take_back)
            .on_hero_turn_start(turn_start)
            .on_hero_option_chosen(choose_power),
    )]
}

/// Start of every turn, whatever Hero Power the hero has now: `Power of the Storm` comes back.
fn take_back(state: &mut TavernState, pool: &mut CardPool) {
    if let Some(nguyen) = super::data(ID) {
        super::replace_power(state, super::new_power_state(nguyen.power), false, pool);
    }
}

/// Start of every turn: offer 2 new Hero Powers.
fn turn_start(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    super::offer_power_discover(state, OPTIONS, None, true, false, rng);
}

/// The chosen Hero Power replaces `Power of the Storm` for this turn.
fn choose_power(
    state: &mut TavernState,
    chosen: Unit,
    _: Vec<Unit>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    super::install_power(state, chosen.card_id, false, pool, rng);
}
