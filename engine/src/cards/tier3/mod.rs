//! Solo Tier 3 card module registry (Patch 36.6.3).
//!
//! All Tavern and Combat hook dispatch lives uniformly in [`crate::cards`].

pub mod abyssal_envoy;
pub mod accord_o_tron;
pub mod amber_guardian;
pub mod annoy_o_module;
pub mod auto_accelerator;
pub mod azsharan_cutlassier;
pub mod blue_whelp;
pub mod cadaver_caretaker;
pub mod deadly_spore;
pub mod devout_hellcaller;
pub mod diremuck_forager;
pub mod disguised_graverobber;
pub mod drifting_sacrifice;
pub mod fearless_foodie;
pub mod fetid_corroder;
pub mod fruit_vendor;
pub mod gem_rat;
pub mod greedy_conniver;
pub mod handless_forsaken;
pub mod hired_mount;
pub mod iron_groundskeeper;
pub mod locked_up_mutineer;
pub mod malchezaar_prince_of_dance;
pub mod mangled_bandit;
pub mod mummifier;
pub mod prosthetic_hand;
pub mod relentless_deflector;
pub mod rescue_bot;
pub mod roaring_recruiter;
pub mod shoalfin_mystic;
pub mod sly_infiltrator;
pub mod sprightly_scarab;
pub mod tasty_lobster;
pub mod thorned_trailblazer;
pub mod timecapn_hooktail;
pub mod trapped_clapper;
pub mod treasure_parrot;
pub mod trench_fighter;
pub mod unwilling_slacker;
pub mod vicious_mindslasher;
pub mod waveling;
pub mod wildfire_elemental;
pub mod wolf_pup;

use crate::cards::CardTemplate;

/// Returns all 43 active Solo Tier 3 templates (Patch 36.6.3).
pub fn catalog() -> Vec<CardTemplate> {
    vec![
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
    ]
}
