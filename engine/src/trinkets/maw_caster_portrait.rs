//! `Maw Caster Portrait` (`BG32_MagicItem_205`) — Greater, 5 Gold, Undead.
//!
//! Get a Maw Caster. Whenever you destroy a minion outside combat, gain 2 Gold.
//!
//! Picked: a plain `Maw Caster`. `trinket_tavern_destroy`: each friendly minion destroyed in the
//! Tavern (`TavernState::destroy_board_unit`: `Maw Caster`, `Jailer Sticker`, ...) gives 2 Gold.

use crate::cards::minions::maw_caster;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 121558;

/// The Gold each destroy gives.
const GOLD: u32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_tavern_destroy(tavern_destroy),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, maw_caster::ID);
}

fn tavern_destroy(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    state.gold += GOLD;
}
