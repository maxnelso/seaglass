//! `Lichling Hoarder` (`BG36_848`) — Tier 5 Undead (`6/9`).
//!
//! Avenge (3): Get a (`2` if Golden) plain copy(ies) of a minion that started in your warband this combat.

use crate::cards::{instantiate_plain_copy, sync_unit_auras, CardTemplate};
use crate::model::{CardId, PlayerAuras, Tribe, Unit};
use crate::rng::Rng;

pub const ID: CardId = 530;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Lichling Hoarder", 6, 9, 5).with_tribe(Tribe::Undead)
}

pub fn on_friendly_death(
    unit: &mut Unit,
    starting_board: &[Unit],
    hand: &mut Vec<Unit>,
    hand_summoned: &mut Vec<bool>,
    auras: &PlayerAuras,
    rng: &mut Rng,
) {
    if unit.card_id != ID || unit.health <= 0 || starting_board.is_empty() {
        return;
    }
    unit.avenge_counter += 1;
    if unit.avenge_counter >= 3 {
        unit.avenge_counter -= 3;
        let count = if unit.is_golden { 2 } else { 1 };
        for _ in 0..count {
            if hand.len() >= 10 {
                break;
            }
            let pick = if starting_board.len() == 1 {
                0
            } else {
                rng.below(starting_board.len())
            };
            let mut copy = instantiate_plain_copy(&starting_board[pick]);
            sync_unit_auras(&mut copy, auras);
            hand.push(copy);
            hand_summoned.push(false);
        }
    }
}
