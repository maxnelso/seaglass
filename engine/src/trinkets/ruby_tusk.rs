//! `Ruby Tusk` (`BG36_MagicItem_409`) — Lesser, 0 Gold, Quilboar.
//!
//! After you play 10 Blood Gems from hand, replace this with a random Greater Quilboar Trinket. (10 left!)
//!
//! `trinket_after_spell`: each Blood Gem played from the hand counts once (`counter`, across
//! turns; not its extra casts, nor the Gems cards play); at 10 it is replaced by a random Greater
//! Quilboar trinket the player doesn't own (`trinkets::replace_with_random`,
//! `docs/trinkets.md` §3.5), at once.

use crate::cards::{self, CardFlags, CardHooks};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};
use crate::trinkets::SpellCast;

pub const ID: CardId = 134437;

/// Blood Gems played until the replacement.
const GEMS: i32 = 10;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_spell(after_spell))]
}

fn after_spell(state: &mut TavernState, cast: &SpellCast, pool: &mut CardPool, rng: &mut Rng) {
    if !cast.from_hand || !cards::hooks(cast.card.card_id).has(CardFlags::BLOOD_GEM) {
        return;
    }
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        t.counter = (t.counter + 1).min(GEMS);
    }
    super::replace_at_count(state, ID, GEMS, Tribe::Quilboar, pool, rng);
}
