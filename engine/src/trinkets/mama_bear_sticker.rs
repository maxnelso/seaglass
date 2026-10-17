//! `Mama Bear Sticker` (`BG35_MagicItem_871`) — Lesser, 0 Gold, Beast.
//!
//! Whenever you summon a Beast, give it +6/+6.
//!
//! Every friendly Beast summoned gains +6/+6: in the Tavern (`trinket_tavern_summon`: a minion
//! played, or summoned by a card) and in combat (`trinket_combat_summon`), as `Mama Bear`'s.

use crate::cards::CardHooks;
use crate::model::{CardId, Tribe, TrinketState, Unit};

pub const ID: CardId = 130877;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_tavern_summon(summon)
            .on_trinket_combat_summon(summon),
    )]
}

fn summon(_: &mut TrinketState, summoned: &mut Unit) {
    if summoned.tribe.matches(Tribe::Beast) {
        summoned.add_stats(6, 6);
    }
}
