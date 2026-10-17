//! `Death's Embrace (late)` (`BG36_MidGameEffect_000t29`) — turns 7–10, weight 100.
//!
//! Has +2/+2 for each Deathrattle you've triggered this game. ({1})
//!
//! `gift_aura`: +2/+2 for each friendly Deathrattle triggered this game
//! (`auras.deathrattles_triggered`), wherever it is. Offered only to a player whose Deathrattles
//! triggered are the most triggered of the three counts (Hotfix 36.2.1; `docs/dark_gifts.md` §5.1).
//! A stat gift a Deathrattle minion can be offered.

use crate::cards::{CardFlags, CardHooks};
use crate::model::{CardId, DarkGift, PlayerAuras, Unit};

pub const ID: CardId = 132554;

/// The stats it has for each one.
const PER: i32 = 2;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_GIVES_STATS.union(CardFlags::GIFT_FOR_DEATHRATTLE))
            .on_gift_aura(aura)
            .on_gift_trigger_count(count),
    )]
}

fn count(auras: &PlayerAuras) -> u32 {
    auras.deathrattles_triggered
}

fn aura(_: &Unit, _: &DarkGift, auras: &PlayerAuras) -> (i32, i32) {
    let bonus = PER.saturating_mul(count(auras).min(i32::MAX as u32) as i32);
    (bonus, bonus)
}
