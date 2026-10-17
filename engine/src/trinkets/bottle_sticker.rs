//! `Bottle Sticker` (`BGDUO_MagicItem_005`) — Lesser, 4 Gold, Duos only.
//!
//! Get a Portal in a Bottle. At the start of each turn, get another.
//!
//! Picked, and at each turn start: `Portal in a Bottle` (`BGDUO_113`), a Tavern spell in no pool,
//! to the hand or waiting for room. Cast on a friendly non-Golden minion: it Passes it, as
//! `Portal in a Crystal` (`docs/duos_tavern.md` §8.3).

use crate::cards::spells::targeted;
use crate::cards::{self, CardFlags, CardHooks};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 117936;
/// `Portal in a Bottle`.
pub const SPELL: CardId = 105250;

/// The trinket's and its spell's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            ID,
            CardHooks::EMPTY
                .on_trinket_bought(give)
                .on_trinket_turn_start(give),
        ),
        (
            SPELL,
            targeted(cast)
                .on_spell_target_filter(|unit| !unit.is_golden)
                .with_flags(CardFlags::NOT_IN_POOL),
        ),
    ]
}

fn give(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.add_to_hand_or_wait(super::token_spell(SPELL, "Portal in a Bottle"));
}

fn cast(state: &mut TavernState, _: &Unit, pos: usize, _: &mut CardPool, _: &mut Rng) {
    cards::pass_board_minion(state, pos);
}
