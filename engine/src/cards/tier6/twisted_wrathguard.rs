//! `Twisted Wrathguard` (`BG35_155`) — Tier 6 Demon (`8/8`).
//!
//! After you sell a minion, add a (`2` if Golden) Fodder to your next Refresh.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 625;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Twisted Wrathguard", 8, 8, 6)
        .with_tribe(Tribe::Demon)
        .on_after_friendly_sell(after_friendly_sell)
}

pub fn after_friendly_sell(
    state: &mut TavernState,
    self_idx: usize,
    _: &mut CardPool,
    _: &mut Rng,
) {
    state.auras.fodder_per_refresh[0] += state.board[self_idx].golden_mult() as u32;
}
