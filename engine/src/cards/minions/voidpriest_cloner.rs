//! `Voidpriest Cloner` (`BGDUO_700`) — Tier 3 Aberration (`5/6`), Duos only.
//! After you discard a card, Pass a plain copy of it. (Once per turn.) (Golden `BGDUO_700_G`,
//! `10/12`: twice per turn.)
//!
//! An `after_friendly_discard` observer; its `charges` are its uses left this turn (1, or 2 if
//! Golden; `+1` when made Golden, as `Wanderer Cho`'s). A plain copy is the card's base,
//! non-Golden, unbuffed version (a spell: a new copy of it), Passed for free
//! (`docs/duos_tavern.md` §8.3).

use crate::cards::{self, spells, CardTemplate};
use crate::model::{CardId, TeammateGift, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10307;
pub const NAME: &str = "Voidpriest Cloner";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 5, 6, 3)
        .with_tribe(Tribe::Aberration)
        .on_reset_turn_charges(reset_turn_charges)
        .on_made_golden(|cloner| cloner.charges += 1)
        .on_after_friendly_discard(after_friendly_discard)
}

/// Its uses this turn: 1, or 2 if Golden.
pub fn reset_turn_charges(cloner: &mut Unit) {
    cloner.charges = cloner.golden_mult() as u32;
}

/// A card was discarded: Pass a plain copy of it, if a use is left this turn.
pub fn after_friendly_discard(
    state: &mut TavernState,
    self_idx: usize,
    discarded: &Unit,
    _: &mut CardPool,
    _: &mut Rng,
) {
    let cloner = &mut state.board[self_idx];
    if cloner.charges == 0 {
        return;
    }
    cloner.charges -= 1;
    state
        .auras
        .give_teammate(TeammateGift::Pass(plain_copy(discarded)));
}

/// The card's base, non-Golden, unbuffed version.
fn plain_copy(card: &Unit) -> Unit {
    if card.is_spell {
        spells::spell_by_id(card.card_id).unwrap_or_else(|| card.clone())
    } else {
        cards::plain_instance(card).unwrap_or_else(|| cards::instantiate_plain_copy(card))
    }
}
