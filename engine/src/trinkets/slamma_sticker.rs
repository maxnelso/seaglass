//! `Slamma Sticker` (`BG30_MagicItem_540`) — Greater, 5 Gold, Beast.
//!
//! After you summon a Beast in combat, double its Attack.
//!
//! `trinket_combat_summon`: a Beast summoned on the player's side in combat (Deathrattle and
//! Rally summons, Reborn copies, hand copies, ...) doubles its Attack after the other summon
//! modifiers (the auras, the Hero Power, the friendly-summon observers: `Mama Bear`'s buff first).

use crate::cards::CardHooks;
use crate::model::{CardId, Tribe, TrinketState, Unit};

pub const ID: CardId = 112381;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_combat_summon(combat_summon))]
}

fn combat_summon(_: &TrinketState, summoned: &mut Unit) {
    if summoned.tribe.matches(Tribe::Beast) {
        let attack = summoned.attack.max(0);
        summoned.add_stats(attack, 0);
    }
}
