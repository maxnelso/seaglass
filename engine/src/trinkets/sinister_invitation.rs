//! `Sinister Invitation` (`BG36_MagicItem_610`) — Greater, 4 Gold.
//!
//! Discover a Golden minion of your most common type with a Dark Gift. It doesn't give a Triple Reward.
//!
//! As picked: a Dark Gift offer (`docs/dark_gifts.md` §12.3) by the button's Tiers for the turn,
//! all of the player's most common type as far as the pool has them, each made Golden as an
//! intrinsic Golden (no Triple Reward, never in a Triple) and with a different gift by the turn's
//! rules (engine decision). Offered only in games with Dark Gifts (`CardFlags::DARK_GIFTS`).

use crate::cards::{CardFlags, CardHooks};
use crate::dark_gifts::{self, GiftPool, OfferRules, TypeRule};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134748;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::DARK_GIFTS)
            .on_trinket_bought(discover),
    )]
}

fn discover(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let rules = OfferRules {
        turn: state.turn,
        tiers: dark_gifts::button_tiers(state.turn).to_vec(),
        most_common: TypeRule::All,
        gifts: GiftPool::Turn,
        golden: true,
    };
    dark_gifts::discover(state, pool, rng, &rules);
}
