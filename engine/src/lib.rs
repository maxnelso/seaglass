//! `seaglass`: Deterministic headless Hearthstone Battlegrounds game engine.
//!
//! Models both the **Tavern (Recruit) Phase** (`docs/tavern.md`) and the
//! **Combat Phase** (`docs/combat.md`).

pub mod cards;
pub mod combat;
pub mod events;
pub mod model;
pub mod rng;
pub mod scenario;
pub mod sim;
pub mod tavern;

#[cfg(feature = "python")]
pub mod python;

pub use cards::{
    catalog_for, full_catalog, solo_tier_3_catalog, solo_tier_4_catalog, solo_tier_5_catalog,
    solo_tier_6_catalog, solo_tier_7_catalog, tier1_catalog, tier2_catalog, tier3_catalog,
    tier4_catalog, tier5_catalog, tier6_catalog, tier7_catalog, ActivateTargetKind, CardTemplate,
};
pub use combat::{resolve_battle, BattleResult};
pub use events::Event;
pub use model::{
    BattleOutcome, CardId, DeityKind, DeityState, GameState, Keyword, PlayerAuras, Side, Tribe,
    Unit, UnitId,
};
pub use rng::Rng;
pub use scenario::{parse_unit, teams_and_state, Defaults, Matchup};
pub use sim::{simulate, simulate_batch, BattleDistribution, DamageStats};
pub use tavern::{
    base_copies_for_tier, base_upgrade_cost, shop_capacity, CardPool, TavernAction, TavernState,
};
