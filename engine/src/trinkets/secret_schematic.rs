//! `Secret Schematic` (`BG36_MagicItem_840`) — Greater, 0 Gold, Mech.
//!
//! After you buy a Mech, get a random Tavern spell that costs (2) or more.
//!
//! `trinket_after_buy`: each Mech bought gives a random Tavern spell of the player's Tier or lower
//! whose price is 2 or more (`darkmoon::random_shop_spell`, as `Unfurled Codex`), which waits for
//! room in a full hand.

use crate::cards::CardHooks;
use crate::darkmoon;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133068;

/// The lowest price of the spell it gives.
const MIN_COST: u32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_buy(after_buy))]
}

fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, rng: &mut Rng) {
    if !bought.is_spell && bought.tribe.matches(Tribe::Mech) {
        if let Some(spell) = darkmoon::random_shop_spell(state, |s| s.spell_cost >= MIN_COST, rng) {
            state.add_to_hand_or_wait(spell);
        }
    }
}
