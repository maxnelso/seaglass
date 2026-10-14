//! `Storm Splitter` (`BGDUO_122`) — Tier 5 Naga (`5/5`), Duos only.
//! Once per turn, after you Pass a Tavern spell, get a new copy of it (Golden `BGDUO_122_G`,
//! `10/10`: 2 new copies).
//!
//! In no pool: Naga is in no lobby (`docs/duos_tavern.md` §8). An `after_pass` observer: after its
//! player Passes a Tavern spell (themselves or by a card), it gets a new copy of the spell, once
//! per turn (`charges`). Blood Gems and Spellcraft spells are not Tavern spells
//! (`docs/duos_tavern.md` §8.3).

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10504;
pub const NAME: &str = "Storm Splitter";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 5, 5, 5)
        .with_tribe(Tribe::Naga)
        .on_reset_turn_charges(|splitter| splitter.charges = 1)
        .on_after_pass(after_pass)
}

/// The player Passed `passed`: a new copy of it (2 if Golden) if it is a Tavern spell, once per
/// turn.
pub fn after_pass(
    state: &mut TavernState,
    self_idx: usize,
    passed: &Unit,
    _: &mut CardPool,
    _: &mut Rng,
) {
    if !passed.is_spell || !spells::is_tavern_spell(passed.card_id) {
        return;
    }
    let splitter = &mut state.board[self_idx];
    if splitter.charges == 0 {
        return;
    }
    splitter.charges -= 1;
    let copies = splitter.golden_mult();
    for _ in 0..copies {
        if let Some(spell) = spells::spell_by_id(passed.card_id) {
            state.add_to_hand(spell);
        }
    }
}
