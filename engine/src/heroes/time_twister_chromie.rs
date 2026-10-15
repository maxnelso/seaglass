//! `Time Twister Chromie` (`BG34_HERO_001`) — Armor 14 (Duos 12).
//!
//! Hero Power: `Mana Per Minute` (`BG34_HERO_001p`), 0 Gold: Refresh the Tavern with Tavern spells.
//!
//! Active, untargeted, once per turn: a Refresh (`TavernState::refresh_shop_with_spells`) whose
//! every slot (the minion slots and the spell slot) holds a Tavern spell drawn as the shop draws
//! one (engine decision), with the after-Refresh effects of any Refresh.

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 126537;
pub const POWER: CardId = 126538;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_power(mana_per_minute))]
}

/// Using it: a Refresh with Tavern spells.
fn mana_per_minute(state: &mut TavernState, _: Option<usize>, pool: &mut CardPool, rng: &mut Rng) {
    state.refresh_shop_with_spells(pool, rng);
}
