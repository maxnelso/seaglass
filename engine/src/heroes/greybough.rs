//! `Greybough` (`TB_BaconShop_HERO_95`) — Armor 16 (Duos 14).
//!
//! Hero Power: `Sprout It Out!` (`TB_BaconShop_HP_107`), 0 Gold: Give +1/+2 and Taunt to minions you summon during combat.
//!
//! Passive. Every minion summoned on the player's side in combat (`hero_combat_summon`:
//! Deathrattle and Rally summons, Reborn copies, hand copies, the Deity, Hero Power summons; engine
//! decision) gains +1/+2 and Taunt as it is summoned.

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId, HeroState, Keyword, Unit};

pub const ID: HeroId = 67553;
pub const POWER: CardId = 67554;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_combat_summon(sprout))]
}

/// A friendly minion is summoned in combat: it gains +1/+2 and Taunt.
fn sprout(_: &HeroState, summoned: &mut Unit) {
    summoned.add_stats(1, 2);
    summoned.apply_keyword(Keyword::Taunt, false);
}
