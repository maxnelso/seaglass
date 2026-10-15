//! `Sir Finley Mrrgglton` (`TB_BaconShop_HERO_40`) — Armor 14 (Duos 12).
//!
//! Hero Power: `Adventure!` (`TB_BaconShop_HP_057`), 0 Gold: At the start of the game, Discover a Hero Power.
//!
//! Passive. At the start of the first turn (`hero_turn_start`), offers a Discover among 3 distinct
//! Hero Powers (`super::offer_power_discover`); the chosen one replaces `Adventure!` for the rest
//! of the game (`super::install_power`).

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 60211;
pub const POWER: CardId = 60450;

/// Discover options.
const OPTIONS: usize = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_turn_start(turn_start)
            .on_hero_option_chosen(choose_power),
    )]
}

/// Start of turn 1: Discover a Hero Power.
fn turn_start(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    if state.turn == 1 {
        super::offer_power_discover(state, OPTIONS, None, false, false, rng);
    }
}

/// The chosen Hero Power replaces `Adventure!`.
fn choose_power(
    state: &mut TavernState,
    chosen: Unit,
    _: Vec<Unit>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    super::install_power(state, chosen.card_id, false, pool, rng);
}
