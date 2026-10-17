//! `Wildfeather Duster` (`BG35_MagicItem_700`) — Greater, 1 Gold, Beast.
//!
//! After you summon 6 Beasts, get a random Beast. (6 left!)
//!
//! Every 6th friendly Beast summoned (`counter`, across turns and combats; in the Tavern, played or
//! summoned by a card, and in combat: `trinket_tavern_summon`, `trinket_combat_summon`) owes a
//! random Beast of the player's Tier or lower (`stacks`, the cards owed). The owed cards come after
//! the Tavern action (`trinket_after_action`), or for a combat's summons at the next turn start
//! (`trinket_turn_start`), not in the combat hand **(engine decision)**.

use crate::cards::CardHooks;
use crate::model::{CardId, Tribe, TrinketState, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130895;

/// Beasts per card.
const BEASTS: i32 = 6;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_combat_summon(summon)
            .on_trinket_tavern_summon(summon)
            .on_trinket_after_action(pay)
            .on_trinket_turn_start(pay),
    )]
}

fn summon(trinket: &mut TrinketState, summoned: &mut Unit) {
    if !summoned.tribe.matches(Tribe::Beast) {
        return;
    }
    trinket.counter += 1;
    if trinket.counter >= BEASTS {
        trinket.counter = 0;
        trinket.stacks += 1;
    }
}

fn pay(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    let Some(trinket) = super::state_mut(&mut state.auras, ID) else {
        return;
    };
    let owed = std::mem::take(&mut trinket.stacks);
    for _ in 0..owed {
        super::give_random_minion(state, rng, |t| t.tribe.matches(Tribe::Beast));
    }
}
