//! `Sindragosa` (`TB_BaconShop_HERO_27`) — Armor 10 (Duos 8).
//!
//! Hero Power: `Stay Frosty` (`TB_BaconShop_HP_014`), 0 Gold: Minions cost (2). The Tavern offers one fewer minion and Freezes at the end of each turn.
//!
//! Passive. `hero_tavern_rules`: minions cost 2 Gold and the shop holds one minion fewer than its
//! capacity (`docs/tavern.md` §3.3). `hero_end_turn`: the Tavern is Frozen, as the player's
//! Freeze (engine decision).

use crate::cards::CardHooks;
use crate::heroes::TavernRules;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 58435;
pub const POWER: CardId = 57945;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_tavern_rules(rules)
            .on_hero_end_turn(freeze),
    )]
}

/// Minions cost 2; one fewer in the shop.
fn rules(rules: &mut TavernRules) {
    rules.minion_cost = 2;
    rules.fewer_shop_minions = 1;
}

/// End of turn: the Tavern Freezes.
fn freeze(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.is_frozen = true;
}
