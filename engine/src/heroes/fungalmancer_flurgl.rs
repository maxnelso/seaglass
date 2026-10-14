//! `Fungalmancer Flurgl` (`TB_BaconShop_HERO_55`) — Armor 12 (Duos 10).
//!
//! Hero Power: `Gone Fishing` (`TB_BaconShop_HP_056`), 0 Gold: After you sell 5 minions, get a random Murloc. (5 left!)
//!
//! Passive. Each minion the player sells counts (`hero_after_sell`, `counter`, across turns);
//! every 5th gives them a random Murloc of their Tavern Tier or lower from the game's catalog
//! (`cards::game_templates`; engine decision), if their hand has room. One `rng.below(k)`.

use crate::cards::{self, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 60372;
pub const POWER: CardId = 60448;

/// Minions to sell per Murloc.
const SALES: i32 = 5;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_after_sell(after_sell))]
}

/// After a sale: every 5th, a random Murloc.
fn after_sell(state: &mut TavernState, _: &Unit, _: &mut CardPool, rng: &mut Rng) {
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    power.counter += 1;
    if power.counter < SALES {
        return;
    }
    power.counter -= SALES;
    let tier = state.tavern_tier;
    let murlocs: Vec<_> = cards::game_templates(&state.auras)
        .iter()
        .filter(|t| t.tavern_tier <= tier && t.tribe.matches(Tribe::Murloc))
        .collect();
    let pick = match murlocs.len() {
        0 => return,
        1 => 0,
        k => rng.below(k),
    };
    let murloc = murlocs[pick].instantiate();
    state.add_to_hand(murloc);
}
