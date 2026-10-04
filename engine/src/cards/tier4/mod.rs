//! Solo Tier 4 card module registry (Patch 36.6.3).
//!
//! All Tavern and Combat hook dispatch lives uniformly in [`crate::cards`].

pub mod air_baller;
pub mod ashen_corruptor;
pub mod banana_slamma;
pub mod bigwig_bandit;
pub mod blade_collector;
pub mod bonker;
pub mod boom_in_a_box;
pub mod bramble_tunneler;
pub mod bream_counter;
pub mod bronze_timewalker;
pub mod cage_gnawer;
pub mod conveyor_construct;
pub mod cutthroat_kthir;
pub mod dark_paradox;
pub mod dead_bellringer;
pub mod drone_duplicator;
pub mod en_djinn_blazer;
pub mod enchanted_sentinel;
pub mod faceless_operative;
pub mod flaming_enforcer;
pub mod friendly_geist;
pub mod gearfin;
pub mod geomagus_roogug;
pub mod glambot;
pub mod gormling_gourmet;
pub mod gunpowder_courier;
pub mod headhunter_gryphon;
pub mod heroic_underdog;
pub mod hoarding_hyena;
pub mod holy_vanguard;
pub mod hot_air_surveyor;
pub mod humongozz;
pub mod ichoron_the_protector;
pub mod imp_lusionist;
pub mod imposing_percussionist;
pub mod kelp_keeper;
pub mod leyline_surfacer;
pub mod living_prison;
pub mod lovesick_balladist;
pub mod maritime_extortionist;
pub mod maw_caster;
pub mod mindbending_recruiter;
pub mod nightmare_corroder;
pub mod parasitic_fleshling;
pub mod persistent_poet;
pub mod plaguerunner;
pub mod razorfen_flapper;
pub mod refreshing_anomaly;
pub mod runic_arcanist;
pub mod sacrificial_wrathguard;
pub mod sindorei_straight_shot;
pub mod sky_hatch_runaway;
pub mod snare_trapper;
pub mod snarky_shark;
pub mod soulkeeping_jailer;
pub mod tavern_tempest;
pub mod tortollan_blue_shell;
pub mod twilight_tidehunter;

use crate::cards::CardTemplate;

/// Returns all 58 active Solo Tier 4 templates (Patch 36.6.3).
pub fn catalog() -> Vec<CardTemplate> {
    vec![
        air_baller::template(),
        ashen_corruptor::template(),
        banana_slamma::template(),
        bigwig_bandit::template(),
        blade_collector::template(),
        bonker::template(),
        boom_in_a_box::template(),
        bramble_tunneler::template(),
        bream_counter::template(),
        bronze_timewalker::template(),
        cage_gnawer::template(),
        conveyor_construct::template(),
        cutthroat_kthir::template(),
        dark_paradox::template(),
        dead_bellringer::template(),
        drone_duplicator::template(),
        en_djinn_blazer::template(),
        enchanted_sentinel::template(),
        faceless_operative::template(),
        flaming_enforcer::template(),
        friendly_geist::template(),
        gearfin::template(),
        geomagus_roogug::template(),
        glambot::template(),
        gormling_gourmet::template(),
        gunpowder_courier::template(),
        headhunter_gryphon::template(),
        heroic_underdog::template(),
        hoarding_hyena::template(),
        holy_vanguard::template(),
        hot_air_surveyor::template(),
        humongozz::template(),
        ichoron_the_protector::template(),
        imp_lusionist::template(),
        imposing_percussionist::template(),
        kelp_keeper::template(),
        leyline_surfacer::template(),
        living_prison::template(),
        lovesick_balladist::template(),
        maritime_extortionist::template(),
        maw_caster::template(),
        mindbending_recruiter::template(),
        nightmare_corroder::template(),
        parasitic_fleshling::template(),
        persistent_poet::template(),
        plaguerunner::template(),
        razorfen_flapper::template(),
        refreshing_anomaly::template(),
        runic_arcanist::template(),
        sacrificial_wrathguard::template(),
        sindorei_straight_shot::template(),
        sky_hatch_runaway::template(),
        snare_trapper::template(),
        snarky_shark::template(),
        soulkeeping_jailer::template(),
        tavern_tempest::template(),
        tortollan_blue_shell::template(),
        twilight_tidehunter::template(),
    ]
}
