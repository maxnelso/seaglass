//! `Trailblazer Sticker` (`BG36_MagicItem_308`) — Greater, 4 Gold, Quilboar.
//!
//! Your Choose One cards have both effects combined.
//!
//! `trinket_extra_triggers` (`Passive::CombinesChooseOne`): every Choose One prompt applies both
//! options at once, as with `combine_choose_one` (`TavernState::resolve_choose_one`).

use crate::cards::{CardHooks, Passive};
use crate::model::CardId;

pub const ID: CardId = 133380;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_extra_triggers(extra))]
}

fn extra(passive: Passive) -> u32 {
    u32::from(passive == Passive::CombinesChooseOne)
}
