//! `seaglass`: Deterministic headless Hearthstone Battlegrounds game engine.
//!
//! Models both the **Tavern (Recruit) Phase** (`docs/tavern.md`, and Duos Taverns:
//! `docs/duos_tavern.md`) and the **Combat Phase** (`docs/combat.md`, and Duos combats:
//! `docs/duos.md`), with heroes and their Hero Powers (`docs/heroes.md`) and trinkets
//! (`docs/trinkets.md`).

pub mod cards;
pub mod combat;
pub mod duos_tavern;
pub mod events;
pub mod heroes;
pub mod lobby;
pub mod model;
pub mod rng;
pub mod scenario;
pub mod sim;
pub mod tavern;
pub mod trinkets;

#[cfg(feature = "python")]
pub mod python;

pub use cards::{
    catalog_for, duos_catalog, duos_exclusive_catalog, full_catalog, solo_tier_3_catalog,
    solo_tier_4_catalog, solo_tier_5_catalog, solo_tier_6_catalog, solo_tier_7_catalog,
    tier1_catalog, tier2_catalog, tier3_catalog, tier4_catalog, tier5_catalog, tier6_catalog,
    tier7_catalog, ActivateTargetKind, CardTemplate,
};
pub use combat::{
    resolve_battle, resolve_duos_battle, BattleResult, DuosResult, DuosTeam, Warband, WarbandResult,
};
pub use duos_tavern::DuosTavern;
pub use events::Event;
pub use model::{
    BattleOutcome, CardId, DeityKind, DeityState, GameState, HeroId, HeroPowerState, HeroState,
    Keyword, LobbyView, OpponentView, PlayerAuras, Side, TeammateGift, Tribe, TrinketOption,
    TrinketState, Unit, UnitId,
};
pub use rng::Rng;
pub use scenario::{parse_unit, teams_and_state, Defaults, Matchup};
pub use sim::{
    simulate, simulate_batch, simulate_duos, simulate_duos_batch, BattleDistribution, DamageStats,
};
pub use tavern::{
    base_copies_for_tier, base_upgrade_cost, shop_capacity, CardPool, MinionSlot, TavernAction,
    TavernState, MAX_SHOP_SIZE,
};
