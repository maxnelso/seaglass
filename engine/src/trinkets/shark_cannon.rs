//! `Shark Cannon` (`BG32_MagicItem_232`) — Greater, 0 Gold, Pirate.
//!
//! After you spend {4} Gold, give your Pirates +1/+1 and improve this. (7 Gold left!)
//!
//! The data's numbers are broken (confirmed): after every 7 Gold spent, the Pirates on the board
//! gain `+(1 + stacks)/+(1 + stacks)` and it improves by +1/+1 (+1/+1, then +2/+2, ...); the
//! count starts again after each trigger. `trinket_gold_spent`: each Gold counts (`counter`,
//! across turns), so a spending that passes 7 keeps the rest toward the next **(engine
//! decision)**.

use crate::cards::CardHooks;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 120939;

/// Gold per trigger.
const GOLD: i32 = 7;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_gold_spent(gold_spent))]
}

fn gold_spent(state: &mut TavernState, amount: u32, _: &mut CardPool, _: &mut Rng) {
    let Some(t) = super::state_mut(&mut state.auras, ID) else {
        return;
    };
    t.counter = t.counter.saturating_add(amount.min(i32::MAX as u32) as i32);
    while let Some(t) = super::state_mut(&mut state.auras, ID).filter(|t| t.counter >= GOLD) {
        t.counter -= GOLD;
        t.stacks = t.stacks.saturating_add(1);
        let bonus = t.stacks;
        for unit in &mut state.board {
            if unit.tribe.matches(Tribe::Pirate) {
                unit.add_stats(bonus, bonus);
            }
        }
    }
}
