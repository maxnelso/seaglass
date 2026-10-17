//! `Tableware Portrait` (`BG36_MagicItem_611`) — Greater, 4 Gold.
//!
//! Get a Menagerie Tableware. At the start of each turn, get another.
//!
//! Picked, and at each turn start: a fresh `Menagerie Tableware` (the Tier 7 Tavern spell), to the
//! hand, or waiting for room in a full hand (`docs/trinkets.md` §5).

use crate::cards::{spells, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134641;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(give)
            .on_trinket_turn_start(give),
    )]
}

fn give(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    if let Some(spell) = spells::spell_by_id(spells::SPELL_MENAGERIE_TABLEWARE) {
        state.add_to_hand_or_wait(spell);
    }
}
