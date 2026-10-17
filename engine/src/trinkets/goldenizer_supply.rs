//! `Goldenizer Supply` (`BG30_MagicItem_435`) — Lesser, 0 Gold.
//!
//! At the end of every 3 turns, get a Goldenizer. (3 turns left!)
//!
//! `trinket_end_turn`: every 3rd end of turn (`counter`, which advances once a turn; the
//! end-of-turn repeats repeat the payoff: confirmed, `docs/trinkets.md` §6) gives `The Goldenizer`
//! (`BG26_813t`), a token spell, to the hand or waiting for room. Cast on a friendly non-Golden minion: it becomes Golden
//! (`Unit::make_golden`, as `Captain Sanders`').

use crate::cards::spells::targeted;
use crate::cards::{CardFlags, CardHooks};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 112082;
/// `The Goldenizer`.
pub const SPELL: CardId = 98914;

/// End of turns per Goldenizer.
const TURNS: i32 = 3;

/// The trinket's and its spell's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (ID, CardHooks::EMPTY.on_trinket_end_turn(end_turn)),
        (
            SPELL,
            targeted(cast)
                .on_spell_target_filter(|unit| !unit.is_golden)
                .with_flags(CardFlags::NOT_IN_POOL.union(CardFlags::NOT_TAVERN_SPELL)),
        ),
    ]
}

fn end_turn(state: &mut TavernState, repeats: u32, _: &mut CardPool, _: &mut Rng) {
    if super::count_to(&mut state.auras, ID, TURNS) {
        for _ in 0..repeats {
            state.add_to_hand_or_wait(super::token_spell(SPELL, "The Goldenizer"));
        }
    }
}

fn cast(state: &mut TavernState, _: &Unit, pos: usize, _: &mut CardPool, _: &mut Rng) {
    if let Some(unit) = state.board.get_mut(pos).filter(|u| !u.is_golden) {
        unit.make_golden();
        state.sync_all_auras();
    }
}
