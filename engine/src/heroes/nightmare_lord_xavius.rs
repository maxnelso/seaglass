//! `Nightmare Lord Xavius` (`BG36_HERO_105`) — Armor 6 (Duos 6).
//!
//! Hero Power: `Feel Devastation` (`BG36_HERO_105p`), 0 Gold: Every 4 turns, Discover a minion with a Dark Gift. (4 turns left!)
//!
//! Passive. At the start of every 4th turn (`hero_turn_start`: turns 4, 8, 12, ...), a Dark
//! Discovery offer by the button's rules for that turn (confirmed: the same Tiers and filters),
//! free and without using the button (`docs/dark_gifts.md` §12.1). Offered only in games with
//! Dark Gifts (`CardFlags::DARK_GIFTS`).

use crate::cards::{CardFlags, CardHooks};
use crate::dark_gifts::{self, OfferRules};
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 132608;
pub const POWER: CardId = 134010;

/// The turns between two offers.
const EVERY: u32 = 4;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .with_flags(CardFlags::DARK_GIFTS)
            .on_hero_turn_start(turn_start),
    )]
}

/// Every 4th turn: a free Dark Discovery offer.
fn turn_start(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if state.turn == 0 || !state.turn.is_multiple_of(EVERY) {
        return;
    }
    dark_gifts::discover(state, pool, rng, &OfferRules::button(state.turn));
}
