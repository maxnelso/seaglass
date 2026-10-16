//! `Magicfin Sticker` (`BG35_MagicItem_750`) — Greater, 4 Gold, Murloc.
//!
//! After you buy a Tavern spell, get a 1/1 Murloc and teach it that spell. (2 times per turn.)
//!
//! `trinket_after_buy`: the first 2 Tavern spells bought each turn (`armed`, which starts again
//! each turn) each give a `Magicfin Apprentice` taught that spell (as `Magicfin Mycologist`'s),
//! which waits for room in a full hand.

use crate::cards::{spells, tokens, CardHooks};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131303;

/// Murlocs per turn.
const PER_TURN: u32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_after_buy(after_buy),
    )]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_turn(state, ID);
}

fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, _: &mut Rng) {
    if !bought.is_spell || !spells::is_tavern_spell(bought.card_id) {
        return;
    }
    let Some(t) = super::state_mut(&mut state.auras, ID).filter(|t| t.armed < PER_TURN) else {
        return;
    };
    t.armed += 1;
    state.add_to_hand_or_wait(tokens::make_magicfin_apprentice(bought.card_id));
}
