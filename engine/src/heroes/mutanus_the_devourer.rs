//! `Mutanus the Devourer` (`BG20_HERO_301`) — Armor 20 (Duos 17).
//!
//! Hero Power: `Devour` (`BG20_HERO_301p`), 0 Gold: Sell a friendly minion. Spit its stats onto another.
//!
//! Active, a friendly minion, once per turn: the minion is sold (`TavernState::sell_minion`, as by
//! the `Sell` action: 1 Gold, its sell triggers, the sell observers, the teammate's in Duos), then
//! a random other friendly minion (one `rng.below(k)`) gains its Attack and Health. It needs
//! another friendly minion (engine decision).

use crate::cards::{ActivateTargetKind, CardHooks};
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 71916;
pub const POWER: CardId = 71914;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(devour)
            .on_hero_power_usable(|state| state.board.len() >= 2)
            .with_activate_target(ActivateTargetKind::BoardAny),
    )]
}

/// Using it: sell the minion, then its stats onto another.
fn devour(state: &mut TavernState, target: Option<usize>, pool: &mut CardPool, rng: &mut Rng) {
    let Some(t) = target.filter(|&t| t < state.board.len()) else {
        return;
    };
    let sold = state.sell_minion(t, pool, rng);
    let pick = match state.board.len() {
        0 => return,
        1 => 0,
        k => rng.below(k),
    };
    state.board[pick].add_stats(sold.attack.max(0), sold.health.max(0));
}
