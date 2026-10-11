//! The Ballers (`Fire Baller`, `Snow Baller` and `Air Baller`): selling any of them improves all
//! future Ballers, through one shared card counter.

use crate::cards::minions;
use crate::model::{CardId, PlayerAuras};

/// Card counter shared by all Ballers: how much future Ballers are improved. Card counters are
/// keyed by a card, so this one uses `Fire Baller`'s id.
pub const BALLER_COUNTER: CardId = minions::fire_baller::ID;

/// How much future Ballers are improved ([`BALLER_COUNTER`]).
pub fn baller_bonus(auras: &PlayerAuras) -> i32 {
    auras.counter(BALLER_COUNTER)
}

/// Improve future Ballers by `n`.
pub fn improve_ballers(auras: &mut PlayerAuras, n: i32) {
    auras.add_counter(BALLER_COUNTER, n);
}
