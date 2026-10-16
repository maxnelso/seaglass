//! `Tess Greymane` (`TB_BaconShop_HERO_50`) — Armor 17 (Duos 15).
//!
//! Hero Power: `Bob's Burgles` (`TB_BaconShop_HP_077`), 1 Gold: Refresh the Tavern with plain copies of your last opponent's warband.
//!
//! Active, untargeted, once per turn: Refreshes the Tavern with plain copies of minions from
//! `state.last_opponent_warband()`, without drawing from `CardPool`. Not usable while that
//! warband is empty (or unknown): it would only empty the Tavern (engine decision).

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 60367;
pub const POWER: CardId = 63162;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(bobs_burgles)
            .on_hero_power_usable(|state| !state.last_opponent_warband().is_empty()),
    )]
}

/// Using it: Refresh the Tavern with plain copies of the last opponent's warband.
fn bobs_burgles(state: &mut TavernState, _: Option<usize>, pool: &mut CardPool, rng: &mut Rng) {
    let warband: Vec<Unit> = state.last_opponent_warband().to_vec();
    state.refresh_shop_with_warband(&warband, pool, rng);
}
