//! `Secret Schematic` (`BG36_MagicItem_840`) — Greater, 0 Gold, Mech.
//!
//! After you buy a Mech, get a random Tavern spell.
//!
//! `trinket_after_buy`: each Mech bought gives a random Tavern spell, drawn as the shop draws one,
//! which waits for room in a full hand.

use crate::cards::{spells, CardHooks};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133068;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_buy(after_buy))]
}

fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, rng: &mut Rng) {
    if !bought.is_spell && bought.tribe.matches(Tribe::Mech) {
        let spell = spells::draw_shop_spell(state.tavern_tier, state.auras.duos_game, rng);
        state.add_to_hand_or_wait(spell);
    }
}
