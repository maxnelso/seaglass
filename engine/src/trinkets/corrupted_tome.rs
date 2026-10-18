//! `Corrupted Tome` (`BG35_MagicItem_812`) — Greater, 2 Gold.
//!
//! Get a Triple Prize. Whenever you would get a Triple Reward, get a Triple Prize instead.
//!
//! Picked: a `Triple Prize` (`BG35_MagicItem_812t`), to the hand or waiting for room.
//! `trinket_replace_gained`: every `Triple Reward` card about to enter the player's hand, from
//! any source (a Golden played, a Hero Power, a teammate; in the Tavern or the combat hand), is a
//! `Triple Prize` instead (engine decision: any source; `docs/darkmoon_prizes.md` §7.3).
//! `Triple Prize`: a 0-Gold token spell, not a Tavern spell, in no pool: a Tier 3 prize Discover,
//! whatever the Tavern Tier (confirmed).

use crate::cards::spells::spell;
use crate::cards::{tokens, CardFlags, CardHooks};
use crate::darkmoon;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130830;
/// `Triple Prize`.
pub const SPELL: CardId = 130856;

/// The Triple Prize's prize Tier (confirmed).
const TIER: u32 = 3;

/// The trinket's and its spell's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            ID,
            CardHooks::EMPTY
                .on_trinket_bought(give)
                .on_trinket_replace_gained(replace),
        ),
        (
            SPELL,
            spell(cast).with_flags(CardFlags::NOT_TAVERN_SPELL.union(CardFlags::NOT_IN_POOL)),
        ),
    ]
}

/// A new `Triple Prize`.
pub fn triple_prize() -> Unit {
    super::token_spell(SPELL, "Triple Prize")
}

fn give(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.add_to_hand_or_wait(triple_prize());
}

fn replace(card: &Unit) -> Option<Unit> {
    (card.card_id == tokens::SPELL_TRIPLE_REWARD).then(triple_prize)
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    darkmoon::discover_prize(state, TIER, pool, rng);
}
