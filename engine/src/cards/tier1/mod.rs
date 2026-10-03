//! Solo Tier 1 card registry and hook routing (Patch 36.6.3).

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
use crate::model::{CardId, PlayerAuras, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

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

pub fn check_stat_thresholds(unit: &mut Unit) {
    scarlet_survivor::check_threshold(unit);
}

pub fn on_play_battlecry(state: &mut TavernState, unit: &mut Unit, rng: &mut Rng) {
    match unit.card_id {
        joyous::ID => joyous::on_battlecry(state, unit),
        ominous_seer::ID => ominous_seer::on_battlecry(state, unit),
        dune_dweller::ID => dune_dweller::on_battlecry(state, unit),
        bubble_gunner::ID => bubble_gunner::on_battlecry(unit, rng),
        southsea_busker::ID => southsea_busker::on_battlecry(state, unit),
        razorfen_geomancer::ID => razorfen_geomancer::on_battlecry(state, unit),
        _ => {}
    }
}

pub fn after_play_minion(
    state: &mut TavernState,
    _played_card_id: CardId,
    played_tribe: Tribe,
    board_pos: usize,
) {
    wrath_weaver::after_play_minion(state, played_tribe, board_pos);
}

pub fn on_sell(state: &mut TavernState, sold: &Unit, _pool: &mut CardPool, _rng: &mut Rng) {
    if sold.card_id == zoatroid::ID {
        zoatroid::on_sell(state, sold);
    }
}

pub fn on_end_turn(state: &mut TavernState) {
    lullabot::on_end_turn(state);
}

pub fn activate_cost(card_id: CardId) -> Option<u32> {
    match card_id {
        suspicious_prisonguard::ID => Some(suspicious_prisonguard::ACTIVATE_COST),
        _ => None,
    }
}

pub fn activate_requires_other_target(card_id: CardId) -> bool {
    matches!(card_id, suspicious_prisonguard::ID)
}

pub fn on_activate(state: &mut TavernState, source_pos: usize, target_pos: Option<usize>) {
    if state.board[source_pos].card_id == suspicious_prisonguard::ID {
        suspicious_prisonguard::on_activate(state, source_pos, target_pos);
    }
}

pub fn on_rally(attacker: &mut Unit) -> Vec<Unit> {
    match attacker.card_id {
        flittering_bat::ID => flittering_bat::on_rally(attacker),
        glim_guardian::ID => glim_guardian::on_rally(attacker),
        tusked_camper::ID => tusked_camper::on_rally(attacker),
        _ => Vec::new(),
    }
}

pub fn on_deathrattle(dying: &Unit, auras: &PlayerAuras) -> Vec<Unit> {
    match dying.card_id {
        buzzing_vermin::ID => buzzing_vermin::on_deathrattle(dying, auras),
        cord_puller::ID => cord_puller::on_deathrattle(dying),
        harmless_bonehead::ID => harmless_bonehead::on_deathrattle(dying, auras),
        _ => Vec::new(),
    }
}

pub fn sync_friendly_death_auras(_board: &mut [Unit], _friendly_deaths_this_combat: u32) {}
