//! `Vibrant Bubble` (`BG32_MagicItem_892`) — Lesser, 3 Gold, Murloc.
//!
//! Spellcraft: Give a Murloc a random Bonus Keyword.
//!
//! Picked, and at each turn start: its Spellcraft spell, `Vibrant Bubble` (`BG32_MagicItem_892t`),
//! a temporary token spell (`docs/trinkets.md` §11.6). Cast on a friendly Murloc: a random Bonus
//! Keyword it doesn't have (as `Bubble Gunner`'s, one `rng.below(k)`); nothing if it has them
//! all.

use crate::cards::spells::{targeted_tribe, SPELLCRAFT};
use crate::cards::CardHooks;
use crate::model::{CardId, Keyword, Tribe, Unit, BONUS_KEYWORDS};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 121138;
/// Its Spellcraft spell.
pub const SPELL: CardId = 121139;

/// The trinket's and its spell's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            ID,
            CardHooks::EMPTY
                .on_trinket_bought(give_spell)
                .on_trinket_turn_start(give_spell),
        ),
        (
            SPELL,
            targeted_tribe(Tribe::Murloc, cast).with_flags(SPELLCRAFT),
        ),
    ]
}

fn give_spell(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.add_to_hand_or_wait(super::spellcraft_spell(SPELL, "Vibrant Bubble"));
}

fn cast(state: &mut TavernState, _: &Unit, pos: usize, _: &mut CardPool, rng: &mut Rng) {
    let Some(murloc) = state.board.get_mut(pos) else {
        return;
    };
    let missing: Vec<Keyword> = BONUS_KEYWORDS
        .iter()
        .copied()
        .filter(|&kw| !murloc.has_keyword(kw))
        .collect();
    if !missing.is_empty() {
        murloc.apply_keyword(missing[rng.below(missing.len())], false);
    }
}
