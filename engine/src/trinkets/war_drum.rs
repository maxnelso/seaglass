//! `War Drum` (`BG32_MagicItem_416`) — Lesser, 2 Gold, Dragon.
//!
//! One Battlecry each turn triggers two extra times. (1 left!)
//!
//! `trinket_extra_battlecries`: the first Battlecry each turn (played or triggered in the Tavern;
//! `armed`, which starts again each turn) triggers 2 more times, added to `Brann Bronzebeard`'s
//! (**engine decision**).

use crate::cards::CardHooks;
use crate::model::{CardId, TrinketState, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 121037;

/// The extra triggers.
const EXTRA: u32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_extra_battlecries(extra),
    )]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_turn(state, ID);
}

fn extra(trinket: &mut TrinketState, _: &Unit) -> u32 {
    if trinket.armed != 0 {
        return 0;
    }
    trinket.armed = 1;
    EXTRA
}
