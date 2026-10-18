//! `Yogg-Tastic Pastry` (`BG30_MagicItem_994`) — Lesser, 5 Gold.
//!
//! Spin the Wheel of Yogg-Saron. At the start of each turn, spin it again.
//!
//! Picked, and at each turn start (`trinket_turn_start`): a spin of the Wheel (`yogg::spin_wheel`:
//! one weighted draw among its 6 outcomes, which resolves at once; `docs/wheel_of_yogg.md` §5).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};
use crate::yogg;

pub const ID: CardId = 113104;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(spin)
            .on_trinket_turn_start(spin),
    )]
}

fn spin(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    yogg::spin_wheel(state, pool, rng);
}
