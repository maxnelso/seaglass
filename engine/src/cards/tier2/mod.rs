//! Solo Tier 2 card templates (Patch 36.6.3).

pub mod bilgewater_breakout;
pub mod blue_volumizer;
pub mod brain_rotter;
pub mod bronze_warden;
pub mod clever_castaway;
pub mod crater_miner;
pub mod decoy_conjurer;
pub mod electric_synthesizer;
pub mod eternal_knight;
pub mod expert_aviator;
pub mod fire_baller;
pub mod forest_rover;
pub mod green_volumizer;
pub mod humming_bird;
pub mod intrepid_botanist;
pub mod laboratory_assistant;
pub mod lurking_lionfish;
pub mod mechagnome_interpreter;
pub mod mind_muck;
pub mod nerubian_deathswarmer;
pub mod patient_scout;
pub mod prodigious_tusker;
pub mod red_volumizer;
pub mod roadboar;
pub mod scarlet_skull;
pub mod sellemental;
pub mod snow_baller;
pub mod soul_rewinder;
pub mod surfing_sylvar;
pub mod tad;
pub mod tarecgosa;
pub mod underrot_spawn;
pub mod very_hungry_winterfinner;
pub mod wandering_willbreaker;

use crate::cards::CardTemplate;

/// Returns all 34 active Solo Tier 2 templates (Patch 36.6.3).
pub fn catalog() -> Vec<CardTemplate> {
    vec![
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
    ]
}
