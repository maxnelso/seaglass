//! `Reinforced Shield` (`BG30_MagicItem_886`) — Greater, 1 Gold, Undead.
//!
//! Whenever you summon a minion, give it Divine Shield. (5 times per combat.)
//!
//! `trinket_combat_summon`: the first 5 minions summoned on the player's side each combat
//! (`armed`, which starts again every combat) gain Divine Shield; summons in the Tavern don't
//! count **(engine decision**: "per combat"**)**.

use crate::cards::CardHooks;
use crate::model::{CardId, Keyword, TrinketState, Unit};

pub const ID: CardId = 110967;

/// Shields per combat.
const TIMES: u32 = 5;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_combat_start(combat_start)
            .on_trinket_combat_summon(combat_summon),
    )]
}

fn combat_start(trinket: &mut TrinketState) {
    trinket.armed = 0;
}

fn combat_summon(trinket: &mut TrinketState, summoned: &mut Unit) {
    if trinket.armed < TIMES {
        trinket.armed += 1;
        summoned.apply_keyword(Keyword::DivineShield, false);
    }
}
