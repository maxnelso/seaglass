//! Minion card definitions: every active Solo minion (Tavern Tiers 1-7, Patch 36.6.3), one
//! module per card.
//!
//! Each module declares its card's `ID`, `NAME` and `template()` (stats, tribe, keywords and
//! [`CardHooks`](crate::cards::CardHooks)) plus the functions implementing its effects. All
//! Tavern and Combat hook dispatch lives in [`crate::cards`].

pub mod abyssal_envoy;
pub mod accord_o_tron;
pub mod air_baller;
pub mod air_revenant;
pub mod amber_guardian;
pub mod annoy_o_module;
pub mod ashen_corruptor;
pub mod aureate_laureate;
pub mod auto_accelerator;
pub mod auto_reveille;
pub mod azsharan_cutlassier;
pub mod balinda_stonehearth;
pub mod banana_slamma;
pub mod barrier_banshee;
pub mod bigwig_bandit;
pub mod bile_spitter;
pub mod bilgewater_breakout;
pub mod blade_collector;
pub mod blue_volumizer;
pub mod blue_whelp;
pub mod bonker;
pub mod boom_in_a_box;
pub mod brain_rotter;
pub mod bramble_tunneler;
pub mod brann_bronzebeard;
pub mod bream_counter;
pub mod bronze_timewalker;
pub mod bronze_warden;
pub mod bubble_gunner;
pub mod buzzing_vermin;
pub mod cadaver_caretaker;
pub mod cage_gnawer;
pub mod captain_sanders;
pub mod cataclysmic_harbinger;
pub mod champion_of_sargeras;
pub mod charging_czarina;
pub mod choral_mrrrglr;
pub mod clever_castaway;
pub mod conveyor_construct;
pub mod cord_puller;
pub mod costume_enthusiast;
pub mod crackling_cyclone;
pub mod crater_miner;
pub mod crimson_vindicator;
pub mod cutthroat_kthir;
pub mod dark_paradox;
pub mod dark_puppeteer;
pub mod de_volition_ist;
pub mod dead_bellringer;
pub mod deadly_spore;
pub mod deathly_striker;
pub mod deathstrider;
pub mod decoy_conjurer;
pub mod deft_deserter;
pub mod devilish_distractor;
pub mod devout_hellcaller;
pub mod diremuck_forager;
pub mod disguised_graverobber;
pub mod draconic_warden;
pub mod drakkari_enchanter;
pub mod drifting_sacrifice;
pub mod drone_duplicator;
pub mod drustfallen_butcher;
pub mod dune_dweller;
pub mod electric_synthesizer;
pub mod elemental_of_surprise;
pub mod elite_navigator;
pub mod en_djinn_blazer;
pub mod enchanted_sentinel;
pub mod enterprising_escapee;
pub mod eredar_escapist;
pub mod eternal_knight;
pub mod eternal_summoner;
pub mod eternal_tycoon;
pub mod expert_aviator;
pub mod faceless_converter;
pub mod faceless_operative;
pub mod falling_sky_golem;
pub mod fearless_foodie;
pub mod felboar;
pub mod felfire_conjurer;
pub mod fetid_corroder;
pub mod fire_baller;
pub mod firelands_fugitive;
pub mod firescale_hoarder;
pub mod flaming_enforcer;
pub mod flighty_scout;
pub mod flittering_bat;
pub mod forest_rover;
pub mod forsaken_weaver;
pub mod friendly_geist;
pub mod fruit_vendor;
pub mod futurefin;
pub mod gatekeeper_amalgam;
pub mod gearfin;
pub mod gem_rat;
pub mod geomagus_roogug;
pub mod ghastcoiler;
pub mod glambot;
pub mod glim_guardian;
pub mod goldrinn_the_great_wolf;
pub mod gormling_gourmet;
pub mod greedy_conniver;
pub mod green_volumizer;
pub mod gunpowder_courier;
pub mod hackerfin;
pub mod handless_forsaken;
pub mod harbinger_aphlass;
pub mod harmless_bonehead;
pub mod headhunter_gryphon;
pub mod heroic_broodmother;
pub mod heroic_underdog;
pub mod highkeeper_ra;
pub mod hired_mount;
pub mod hoarding_hyena;
pub mod holy_vanguard;
pub mod hooktusk_master_marauder;
pub mod hopebringer;
pub mod hot_air_surveyor;
pub mod humming_bird;
pub mod humongozz;
pub mod ichoron_the_protector;
pub mod imp_lusionist;
pub mod imposing_percussionist;
pub mod insatiable_urzul;
pub mod intrepid_botanist;
pub mod iron_groundskeeper;
pub mod jailbird_juggernaut;
pub mod joyous;
pub mod kalecgos_arcane_aspect;
pub mod kelp_keeper;
pub mod laboratory_assistant;
pub mod leeroy_the_reckless;
pub mod leyline_surfacer;
pub mod lichling_hoarder;
pub mod living_azerite;
pub mod living_prison;
pub mod locked_up_mutineer;
pub mod lovesick_balladist;
pub mod lullabot;
pub mod lurking_leviathan;
pub mod lurking_lionfish;
pub mod magicfin_mycologist;
pub mod malchezaar_prince_of_dance;
pub mod mangled_bandit;
pub mod maritime_extortionist;
pub mod maw_caster;
pub mod mechagnome_interpreter;
pub mod mind_muck;
pub mod mindbender_ghursha;
pub mod mindbending_recruiter;
pub mod mummifier;
pub mod mysterious_kthir;
pub mod nadina_the_red;
pub mod nerubian_deathswarmer;
pub mod nightmare_corroder;
pub mod nightmare_par_tea_guest;
pub mod nraqi_frostcaller;
pub mod nraqi_sapper;
pub mod obsidian_ravager;
pub mod ominous_seer;
pub mod parasitic_fleshling;
pub mod patient_scout;
pub mod persistent_poet;
pub mod plaguerunner;
pub mod polarizing_beatboxer;
pub mod primalfin_lookout;
pub mod prodigious_tusker;
pub mod prosthetic_hand;
pub mod proud_privateer;
pub mod ravaging_scorpid;
pub mod razorfen_flapper;
pub mod razorfen_geomancer;
pub mod razorfen_vineweaver;
pub mod red_volumizer;
pub mod refreshing_anomaly;
pub mod relentless_deflector;
pub mod rescue_bot;
pub mod resourceful_robot;
pub mod risen_rider;
pub mod roadboar;
pub mod roaring_recruiter;
pub mod rodeo_performer;
pub mod runic_arcanist;
pub mod sacrificial_wrathguard;
pub mod sanguine_champion;
pub mod sanguine_refiner;
pub mod scarlet_skull;
pub mod scarlet_survivor;
pub mod sellemental;
pub mod sewer_escapee;
pub mod sewer_lord;
pub mod sha_of_fear;
pub mod shamanic_tidecaller;
pub mod ship_master_eudora;
pub mod shipwrecked_rascal;
pub mod shoalfin_mystic;
pub mod silent_deliverer;
pub mod sindorei_straight_shot;
pub mod sky_admiral_rogers;
pub mod sky_hatch_runaway;
pub mod sly_infiltrator;
pub mod snare_trapper;
pub mod snarky_shark;
pub mod snazzy_phantom;
pub mod snow_baller;
pub mod soul_rewinder;
pub mod soulkeeping_jailer;
pub mod southsea_busker;
pub mod spark_snapper;
pub mod sprightly_scarab;
pub mod stalwart_kodo;
pub mod stitched_salvager;
pub mod stone_age_slab;
pub mod surfing_sylvar;
pub mod suspicious_prisonguard;
pub mod tad;
pub mod tarecgosa;
pub mod tasty_lobster;
pub mod tavern_tempest;
pub mod the_last_one_standing;
pub mod the_shadow_of_doubt;
pub mod thorned_trailblazer;
pub mod tichondrius;
pub mod timecapn_hooktail;
pub mod titus_rivendare;
pub mod tortollan_blue_shell;
pub mod trapped_clapper;
pub mod treasure_parrot;
pub mod trench_fighter;
pub mod turbo_hogrider;
pub mod turquoise_skitterer;
pub mod tusked_camper;
pub mod twilight_tidehunter;
pub mod twisted_wrathguard;
pub mod tyrael;
pub mod ultraviolet_ascendant;
pub mod unbound_tempest;
pub mod underrot_spawn;
pub mod unwilling_slacker;
pub mod utility_drone;
pub mod very_hungry_winterfinner;
pub mod veteran_brigand;
pub mod vicious_mindslasher;
pub mod victorious_geomant;
pub mod wandering_willbreaker;
pub mod waveling;
pub mod wildfire_elemental;
pub mod wolf_pup;
pub mod wrath_weaver;
pub mod young_murk_eye;
pub mod zoatroid;

use crate::cards::CardTemplate;

/// All 252 active Solo minions, grouped by Tavern Tier.
///
/// The order matters: it is the order of the shared card pool and of random picks among a
/// tier's minions, so add new cards at the end of their tier's group.
pub fn catalog() -> Vec<CardTemplate> {
    vec![
        // Tier 1
        joyous::template(),
        zoatroid::template(),
        buzzing_vermin::template(),
        flittering_bat::template(),
        wrath_weaver::template(),
        ominous_seer::template(),
        glim_guardian::template(),
        scarlet_survivor::template(),
        crackling_cyclone::template(),
        dune_dweller::template(),
        cord_puller::template(),
        lullabot::template(),
        bubble_gunner::template(),
        flighty_scout::template(),
        aureate_laureate::template(),
        southsea_busker::template(),
        razorfen_geomancer::template(),
        tusked_camper::template(),
        harmless_bonehead::template(),
        risen_rider::template(),
        suspicious_prisonguard::template(),
        // Tier 2
        bilgewater_breakout::template(),
        blue_volumizer::template(),
        brain_rotter::template(),
        bronze_warden::template(),
        clever_castaway::template(),
        crater_miner::template(),
        decoy_conjurer::template(),
        electric_synthesizer::template(),
        eternal_knight::template(),
        expert_aviator::template(),
        fire_baller::template(),
        forest_rover::template(),
        green_volumizer::template(),
        humming_bird::template(),
        intrepid_botanist::template(),
        laboratory_assistant::template(),
        lurking_lionfish::template(),
        mechagnome_interpreter::template(),
        mind_muck::template(),
        nerubian_deathswarmer::template(),
        patient_scout::template(),
        prodigious_tusker::template(),
        red_volumizer::template(),
        roadboar::template(),
        scarlet_skull::template(),
        sellemental::template(),
        snow_baller::template(),
        soul_rewinder::template(),
        surfing_sylvar::template(),
        tad::template(),
        tarecgosa::template(),
        underrot_spawn::template(),
        very_hungry_winterfinner::template(),
        wandering_willbreaker::template(),
        // Tier 3
        abyssal_envoy::template(),
        accord_o_tron::template(),
        amber_guardian::template(),
        annoy_o_module::template(),
        auto_accelerator::template(),
        azsharan_cutlassier::template(),
        blue_whelp::template(),
        cadaver_caretaker::template(),
        deadly_spore::template(),
        devout_hellcaller::template(),
        diremuck_forager::template(),
        disguised_graverobber::template(),
        drifting_sacrifice::template(),
        fearless_foodie::template(),
        fetid_corroder::template(),
        fruit_vendor::template(),
        gem_rat::template(),
        greedy_conniver::template(),
        handless_forsaken::template(),
        hired_mount::template(),
        iron_groundskeeper::template(),
        locked_up_mutineer::template(),
        malchezaar_prince_of_dance::template(),
        mangled_bandit::template(),
        mummifier::template(),
        prosthetic_hand::template(),
        relentless_deflector::template(),
        rescue_bot::template(),
        roaring_recruiter::template(),
        shoalfin_mystic::template(),
        sly_infiltrator::template(),
        sprightly_scarab::template(),
        tasty_lobster::template(),
        thorned_trailblazer::template(),
        timecapn_hooktail::template(),
        trapped_clapper::template(),
        treasure_parrot::template(),
        trench_fighter::template(),
        unwilling_slacker::template(),
        vicious_mindslasher::template(),
        waveling::template(),
        wildfire_elemental::template(),
        wolf_pup::template(),
        // Tier 4
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
        // Tier 5
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
        // Tier 6
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
        // Tier 7
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
