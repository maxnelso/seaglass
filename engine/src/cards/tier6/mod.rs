//! All 32 active Solo Tier 6 minions (Patch 36.6.3), one file per card.

use crate::cards::CardTemplate;

pub mod auto_reveille;
pub mod balinda_stonehearth;
pub mod choral_mrrrglr;
pub mod crimson_vindicator;
pub mod dark_puppeteer;
pub mod deathly_striker;
pub mod deathstrider;
pub mod elemental_of_surprise;
pub mod eredar_escapist;
pub mod falling_sky_golem;
pub mod forsaken_weaver;
pub mod gatekeeper_amalgam;
pub mod harbinger_aphlass;
pub mod heroic_broodmother;
pub mod hooktusk_master_marauder;
pub mod magicfin_mycologist;
pub mod nadina_the_red;
pub mod ravaging_scorpid;
pub mod sanguine_champion;
pub mod silent_deliverer;
pub mod sky_admiral_rogers;
pub mod snazzy_phantom;
pub mod the_shadow_of_doubt;
pub mod turbo_hogrider;
pub mod twisted_wrathguard;
pub mod tyrael;
pub mod ultraviolet_ascendant;
pub mod unbound_tempest;
pub mod utility_drone;
pub mod veteran_brigand;
pub mod victorious_geomant;
pub mod young_murk_eye;

/// Return all 32 active Solo Tier 6 minion templates.
pub fn catalog() -> Vec<CardTemplate> {
    vec![
        auto_reveille::template(),
        balinda_stonehearth::template(),
        choral_mrrrglr::template(),
        crimson_vindicator::template(),
        dark_puppeteer::template(),
        deathly_striker::template(),
        deathstrider::template(),
        elemental_of_surprise::template(),
        eredar_escapist::template(),
        falling_sky_golem::template(),
        forsaken_weaver::template(),
        gatekeeper_amalgam::template(),
        harbinger_aphlass::template(),
        heroic_broodmother::template(),
        hooktusk_master_marauder::template(),
        magicfin_mycologist::template(),
        nadina_the_red::template(),
        ravaging_scorpid::template(),
        sanguine_champion::template(),
        silent_deliverer::template(),
        sky_admiral_rogers::template(),
        snazzy_phantom::template(),
        the_shadow_of_doubt::template(),
        turbo_hogrider::template(),
        twisted_wrathguard::template(),
        tyrael::template(),
        ultraviolet_ascendant::template(),
        unbound_tempest::template(),
        utility_drone::template(),
        veteran_brigand::template(),
        victorious_geomant::template(),
        young_murk_eye::template(),
    ]
}
