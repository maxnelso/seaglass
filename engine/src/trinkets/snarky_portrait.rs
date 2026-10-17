//! `Snarky Portrait` (`BG36_MagicItem_450`) — Greater, 2 Gold, Beast.
//!
//! Get 2 Snarky Sharks. Whenever a Fishbait dies, give your Beasts +4/+4 and improve this.
//!
//! Picked: 2 plain `Snarky Shark`s. `trinket_shop_minion_died`: each `Fishbait` that dies in the
//! Tavern (a friendly minion attacked it) gives the Beasts on the board `+(4 + 4 × stacks)` each,
//! then improves it by +4/+4 (`stacks`) **(engine decision**: improved by the base amount, as
//! Shark Cannon**)**.

use crate::cards::minions::snarky_shark;
use crate::cards::{tokens, CardHooks};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134637;

/// The stats it gives, and its improvement.
const STATS: i32 = 4;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_shop_minion_died(shop_minion_died),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, snarky_shark::ID);
    super::give_minion(state, snarky_shark::ID);
}

fn shop_minion_died(state: &mut TavernState, dead: &Unit, _: &mut CardPool, _: &mut Rng) {
    if dead.card_id != tokens::TOKEN_FISHBAIT {
        return;
    }
    let Some(t) = super::state_mut(&mut state.auras, ID) else {
        return;
    };
    let buff = STATS.saturating_mul(t.stacks.saturating_add(1));
    t.stacks = t.stacks.saturating_add(1);
    for unit in &mut state.board {
        if unit.tribe.matches(Tribe::Beast) {
            unit.add_stats(buff, buff);
        }
    }
}
