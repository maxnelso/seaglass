//! `Spell Siphon (late)` (`BG36_MidGameEffect_000t30`) — turns 7–10, weight 100.
//!
//! Has +3/+3 for each Tavern spell you've cast this game. ({1})
//!
//! `gift_aura`: +3/+3 for each Tavern spell the player cast this game (`auras.tavern_spells_cast`),
//! wherever it is. Offered only to a player whose Tavern spells cast are the most triggered of the
//! three counts (Hotfix 36.2.1; `docs/dark_gifts.md` §5.1). A stat gift.

use crate::cards::{CardFlags, CardHooks};
use crate::model::{CardId, DarkGift, PlayerAuras, Unit};

pub const ID: CardId = 132555;

/// The stats it has for each one.
const PER: i32 = 3;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_GIVES_STATS)
            .on_gift_aura(aura)
            .on_gift_trigger_count(count),
    )]
}

fn count(auras: &PlayerAuras) -> u32 {
    auras.tavern_spells_cast
}

fn aura(_: &Unit, _: &DarkGift, auras: &PlayerAuras) -> (i32, i32) {
    let bonus = PER.saturating_mul(count(auras).min(i32::MAX as u32) as i32);
    (bonus, bonus)
}
