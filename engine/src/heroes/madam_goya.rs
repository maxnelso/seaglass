//! `Madam Goya` (`BGDUO_HERO_104`) — Duos Armor 8.
//!
//! Hero Power: `Efficient Exchange` (`BGDUO_HERO_104p`), 2 Gold: Pass a non-Golden minion.
//!
//! Active, a friendly non-Golden minion, once per turn, Duos only: the minion is Passed to the
//! teammate as a card Passes one (`cards::pass_board_minion`: it leaves the board with its stats,
//! not back to the pool; `docs/duos_tavern.md` §8.3). Not a Pass action: it doesn't count toward
//! the team's Passes (engine decision).

use crate::cards::{self, ActivateTargetKind, CardHooks};
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 107183;
pub const POWER: CardId = 107184;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(efficient_exchange)
            .on_hero_power_usable(|state| state.auras.duos_game)
            .on_hero_power_target(|state, t| !state.board[t].is_golden)
            .with_activate_target(ActivateTargetKind::BoardAny),
    )]
}

/// Using it: Pass the minion.
fn efficient_exchange(
    state: &mut TavernState,
    target: Option<usize>,
    _: &mut CardPool,
    _: &mut Rng,
) {
    if let Some(t) = target {
        cards::pass_board_minion(state, t);
    }
}
