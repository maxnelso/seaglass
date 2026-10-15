//! `Millhouse Manastorm` (`TB_BaconShop_HERO_49`) — Armor 16 (Duos 14).
//!
//! Hero Power: `Manastorm` (`TB_BaconShop_HP_054`), 0 Gold: Minions and Refreshes cost 2 Gold. Upgrading the Tavern costs (1) more.
//!
//! Passive. `hero_tavern_rules`: minions and paid Refreshes cost 2 Gold, and each Tier's Upgrade
//! starts 1 Gold dearer (`upgrade_cost` starts at the base cost + 1; the turn discounts still lower
//! it, down to 0; `docs/tavern.md` §3.3; confirmed). Getting the hero adds the 1 to the current
//! `upgrade_cost` (`hero_chosen`).

use crate::cards::CardHooks;
use crate::heroes::TavernRules;
use crate::model::{CardId, HeroId};
use crate::tavern::TavernState;

pub const ID: HeroId = 60366;
pub const POWER: CardId = 60405;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_tavern_rules(rules)
            .on_hero_chosen(chosen),
    )]
}

/// Getting the hero: the current Upgrade costs 1 more.
fn chosen(state: &mut TavernState) {
    if state.tavern_tier < 6 {
        state.upgrade_cost += 1;
    }
}

/// Minions and Refreshes cost 2; Upgrading 1 more.
fn rules(rules: &mut TavernRules) {
    rules.minion_cost = 2;
    rules.refresh_cost = 2;
    rules.upgrade_extra = 1;
}
