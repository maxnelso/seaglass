//! `Floating Candle Set` (`BG36_MagicItem_208`) — Lesser, 4 Gold.
//!
//! Spellcraft: Trigger a friendly minion's Deathrattle.
//!
//! Picked, and at each turn start: its Spellcraft spell, `Floating Candle Set`
//! (`BG36_MagicItem_208t`), a temporary token spell (`docs/trinkets.md` §11.6). Cast on a friendly
//! minion with a Deathrattle (its own or one an effect gave it): its Deathrattles trigger in the
//! Tavern (`cards::trigger_tavern_deathrattle`, as `Wolfhead Flail`'s).

use crate::cards::spells::{targeted, SPELLCRAFT};
use crate::cards::{self, CardHooks};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133395;
/// Its Spellcraft spell.
pub const SPELL: CardId = 133396;

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
            targeted(cast)
                .on_spell_target_filter(super::has_deathrattle)
                .with_flags(SPELLCRAFT),
        ),
    ]
}

fn give_spell(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.add_to_hand_or_wait(super::spellcraft_spell(SPELL, "Floating Candle Set"));
}

fn cast(state: &mut TavernState, _: &Unit, pos: usize, pool: &mut CardPool, rng: &mut Rng) {
    cards::trigger_tavern_deathrattle(state, pos, pool, rng);
}
