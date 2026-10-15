//! `Mister Clocksworth` (`BG34_HERO_002`) — Armor 18 (Duos 12).
//!
//! Hero Power: `Double Time` (`BG34_HERO_002p`), 0 Gold: You only need 2 copies to make minions Golden. They give Tavern Coins instead of Triple Rewards.
//!
//! Passive. `hero_tavern_rules`: a Triple takes 2 copies (its Golden returns 2 to the pool), and a
//! Golden played from the hand gives a `Tavern Coin` instead of a `Triple Reward` card, whatever
//! made it Golden (`docs/tavern.md` §3.3, §5; engine decision).

use crate::cards::CardHooks;
use crate::heroes::TavernRules;
use crate::model::{CardId, HeroId};

pub const ID: HeroId = 126532;
pub const POWER: CardId = 126533;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_tavern_rules(rules))]
}

/// Two copies make a Golden, which gives a Tavern Coin.
fn rules(rules: &mut TavernRules) {
    rules.triple_copies = 2;
    rules.golden_gives_triple_reward = false;
}
