//! All 52 active Solo Tier 5 minions (Patch 36.6.3), one file per card.

use crate::cards::CardTemplate;

pub mod air_revenant;
pub mod barrier_banshee;
pub mod bile_spitter;
pub mod brann_bronzebeard;
pub mod cataclysmic_harbinger;
pub mod charging_czarina;
pub mod costume_enthusiast;
pub mod de_volition_ist;
pub mod deft_deserter;
pub mod devilish_distractor;
pub mod draconic_warden;
pub mod drakkari_enchanter;
pub mod drustfallen_butcher;
pub mod elite_navigator;
pub mod enterprising_escapee;
pub mod eternal_summoner;
pub mod eternal_tycoon;
pub mod faceless_converter;
pub mod felboar;
pub mod felfire_conjurer;
pub mod firelands_fugitive;
pub mod firescale_hoarder;
pub mod ghastcoiler;
pub mod goldrinn_the_great_wolf;
pub mod hackerfin;
pub mod hopebringer;
pub mod insatiable_urzul;
pub mod kalecgos_arcane_aspect;
pub mod leeroy_the_reckless;
pub mod lichling_hoarder;
pub mod living_azerite;
pub mod lurking_leviathan;
pub mod mindbender_ghursha;
pub mod mysterious_kthir;
pub mod nightmare_par_tea_guest;
pub mod nraqi_frostcaller;
pub mod nraqi_sapper;
pub mod primalfin_lookout;
pub mod proud_privateer;
pub mod razorfen_vineweaver;
pub mod resourceful_robot;
pub mod rodeo_performer;
pub mod sanguine_refiner;
pub mod sewer_escapee;
pub mod sewer_lord;
pub mod shamanic_tidecaller;
pub mod ship_master_eudora;
pub mod shipwrecked_rascal;
pub mod spark_snapper;
pub mod tichondrius;
pub mod titus_rivendare;
pub mod turquoise_skitterer;

/// Return all 52 active Solo Tier 5 minion templates.
pub fn catalog() -> Vec<CardTemplate> {
    vec![
        air_revenant::template(),
        barrier_banshee::template(),
        bile_spitter::template(),
        brann_bronzebeard::template(),
        cataclysmic_harbinger::template(),
        charging_czarina::template(),
        costume_enthusiast::template(),
        de_volition_ist::template(),
        deft_deserter::template(),
        devilish_distractor::template(),
        draconic_warden::template(),
        drakkari_enchanter::template(),
        drustfallen_butcher::template(),
        elite_navigator::template(),
        enterprising_escapee::template(),
        eternal_summoner::template(),
        eternal_tycoon::template(),
        faceless_converter::template(),
        felboar::template(),
        felfire_conjurer::template(),
        firelands_fugitive::template(),
        firescale_hoarder::template(),
        ghastcoiler::template(),
        goldrinn_the_great_wolf::template(),
        hackerfin::template(),
        hopebringer::template(),
        insatiable_urzul::template(),
        kalecgos_arcane_aspect::template(),
        leeroy_the_reckless::template(),
        lichling_hoarder::template(),
        living_azerite::template(),
        lurking_leviathan::template(),
        mindbender_ghursha::template(),
        mysterious_kthir::template(),
        nightmare_par_tea_guest::template(),
        nraqi_frostcaller::template(),
        nraqi_sapper::template(),
        primalfin_lookout::template(),
        proud_privateer::template(),
        razorfen_vineweaver::template(),
        resourceful_robot::template(),
        rodeo_performer::template(),
        sanguine_refiner::template(),
        sewer_escapee::template(),
        sewer_lord::template(),
        shamanic_tidecaller::template(),
        ship_master_eudora::template(),
        shipwrecked_rascal::template(),
        spark_snapper::template(),
        tichondrius::template(),
        titus_rivendare::template(),
        turquoise_skitterer::template(),
    ]
}
