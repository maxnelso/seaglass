//! `Reinvigorating Light` (`BG36_MagicItem_411`) — Lesser, 0 Gold.
//!
//! Discover a second Hero Power. Gain 3 Gold.
//!
//! Picked: 3 Gold, then a Discover of 3 Hero Powers (`heroes::offer_power_discover`, as `Genn,
//! Worgen King`'s second one: not the hero's current ones, nor those no Discover offers); the
//! chosen one becomes the hero's second Hero Power (`hero.second_power`, used with `HeroPower2`),
//! replacing one the hero already had **(engine decision)**. Without a hero, only the Gold.

use crate::cards::CardHooks;
use crate::heroes;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134325;

/// The Gold it gives.
const GOLD: u32 = 3;
/// The Discover's options.
const OPTIONS: usize = 3;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    state.gold += GOLD;
    if state.auras.hero.is_some() {
        heroes::offer_power_discover(state, OPTIONS, None, false, true, rng);
    }
}
