//! Solo Tier 1 card templates (Patch 36.6.3).

pub mod aureate_laureate;
pub mod bubble_gunner;
pub mod buzzing_vermin;
pub mod cord_puller;
pub mod crackling_cyclone;
pub mod dune_dweller;
pub mod flighty_scout;
pub mod flittering_bat;
pub mod glim_guardian;
pub mod harmless_bonehead;
pub mod joyous;
pub mod lullabot;
pub mod ominous_seer;
pub mod razorfen_geomancer;
pub mod risen_rider;
pub mod scarlet_survivor;
pub mod southsea_busker;
pub mod suspicious_prisonguard;
pub mod tusked_camper;
pub mod wrath_weaver;
pub mod zoatroid;

use crate::cards::CardTemplate;

/// Returns all 21 active Solo Tier 1 templates (Patch 36.6.3).
pub fn catalog() -> Vec<CardTemplate> {
    vec![
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
    ]
}
