//! `Master Nguyen` (`BG20_HERO_202`) — Armor 10 (Duos 8).
//!
//! Hero Power: `Power of the Storm` (`BG20_HERO_202p`), 0 Gold: At the start of every turn, choose from 2 new Hero Powers.
//!
//! Passive. At the start of every turn, `super::turn_reset` restores `Power of the Storm`, and
//! `turn_start` offers a Discover of 2 new Hero Powers for that turn.

use crate::cards::CardHooks;
use crate::heroes::power_mut;
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
            .on_hero_turn_start(turn_start)
            .on_hero_option_chosen(choose_power),
    )]
}

/// Start of every turn: return any minion kept from a previous turn's `Lock and Load` to the pool,
/// then offer 2 new Hero Powers.
fn turn_start(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(power) = power_mut(&mut state.auras) {
        for minion in std::mem::take(&mut power.stored) {
            pool.return_unit(&minion);
        }
    }
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
