//! `Ancient Wishbone` (`BG30_MagicItem_804`) — Lesser, 0 Gold.
//!
//! Your Hero Power triggers twice.
//!
//! `trinket_hero_power_triggers`: each use of an active Hero Power (`HeroPower`, and the second
//! one's `HeroPower2`) resolves its effect once more, on the same target if it is still a valid
//! one; its cost is paid once. A passive Hero Power's effects don't repeat **(engine decision)**.

use crate::cards::CardHooks;
use crate::model::{CardId, TrinketState};

pub const ID: CardId = 110957;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_hero_power_triggers(extra_triggers),
    )]
}

fn extra_triggers(_: &TrinketState) -> u32 {
    1
}
