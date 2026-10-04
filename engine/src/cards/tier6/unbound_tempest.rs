//! `Unbound Tempest` (`BG36_352`) — Tier 6 Elemental (`3/12`).
//!
//! After you play 4 Elementals, gain (`double` if Golden) the stats of the highest-Health minion in the Tavern.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 628;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Unbound Tempest", 3, 12, 6).with_tribe(Tribe::Elemental)
}

pub fn after_play_minion(state: &mut TavernState, played_tribe: Tribe, board_pos: usize) {
    if !played_tribe.matches(Tribe::Elemental) {
        return;
    }
    let best_shop = state
        .shop
        .iter()
        .filter(|u| !u.is_spell)
        .max_by_key(|u| (u.health, u.attack))
        .map(|u| (u.attack.max(0), u.health.max(0)));
    for (idx, u) in state.board.iter_mut().enumerate() {
        if u.card_id == ID && idx != board_pos {
            u.unbound_tempest_progress += 1;
            while u.unbound_tempest_progress >= 4 {
                u.unbound_tempest_progress -= 4;
                if let Some((atk, hp)) = best_shop {
                    let mult = if u.is_golden { 2 } else { 1 };
                    u.add_stats(atk * mult, hp * mult);
                }
            }
        }
    }
}
