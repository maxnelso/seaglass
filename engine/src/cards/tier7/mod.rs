//! Tier 7 card definitions (`Patch 36.6.3`).
//!
//! Each active Solo Tier 7 minion lives in its own module with its `CardTemplate`
//! constructor and effect logic. All Tavern and Combat hooks are dispatched
//! through the unified tier-agnostic tables in `super` (`src/cards/mod.rs`).

pub mod captain_sanders;
pub mod champion_of_sargeras;
pub mod futurefin;
pub mod highkeeper_ra;
pub mod jailbird_juggernaut;
pub mod obsidian_ravager;
pub mod polarizing_beatboxer;
pub mod sha_of_fear;
pub mod stalwart_kodo;
pub mod stitched_salvager;
pub mod stone_age_slab;
pub mod the_last_one_standing;

use super::CardTemplate;

/// All 12 active Solo Tier 7 minions in Patch 36.6.3.
pub fn catalog() -> Vec<CardTemplate> {
    vec![
        captain_sanders::template(),
        champion_of_sargeras::template(),
        futurefin::template(),
        highkeeper_ra::template(),
        jailbird_juggernaut::template(),
        obsidian_ravager::template(),
        polarizing_beatboxer::template(),
        sha_of_fear::template(),
        stalwart_kodo::template(),
        stitched_salvager::template(),
        stone_age_slab::template(),
        the_last_one_standing::template(),
    ]
}
