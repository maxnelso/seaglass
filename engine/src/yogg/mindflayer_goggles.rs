//! `Mindflayer Goggles` (`TB_BaconShop_HERO_35_Buddy_t8`) — Wheel of Yogg-Saron, 19%.
//!
//! Cast 4 random Tavern spells.
//!
//! 4 times, one after the other, as Yogg-Saron's `Puzzle Box` casts one
//! (`spells::cast_random_tavern_spell`): a Tavern spell of the player's Tier or lower drawn as the
//! shop draws one, cast by an effect (a spell the player casts, which the spell-cast observers and
//! the trinkets see: confirmed); a targeted one on a random valid target, and with none that one is
//! not cast (engine decision; `docs/wheel_of_yogg.md` §4).

use crate::cards::spells::{self, spell};
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 93391;

/// The Tavern spells it casts (confirmed).
const CASTS: u32 = 4;

/// The outcome's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    for _ in 0..CASTS {
        spells::cast_random_tavern_spell(state, pool, rng);
    }
}
