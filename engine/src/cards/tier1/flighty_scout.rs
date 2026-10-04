//! `Flighty Scout` (`BG32_330`) — Tier 1 Murloc (`3/3`).
//! **Start of Combat:** If this minion is in your hand, summon a copy of it.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 113;
pub const NAME: &str = "Flighty Scout";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 1).with_tribe(Tribe::Murloc)
}

/// Summon copies of any `Flighty Scout` held in `hand` onto `combat_board` at Start of Combat.
/// (Golden `Flighty Scout` summons 2 copies.)
pub fn apply_start_of_combat_hand(hand: &[Unit], combat_board: &mut Vec<Unit>) {
    for card in hand {
        if card.card_id == ID {
            let copies = if card.is_golden { 2 } else { 1 };
            for _ in 0..copies {
                if combat_board.len() < 7 {
                    combat_board.push(card.clone());
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn on_start_of_combat(
    side: crate::model::Side,
    hand: &[Unit],
    board: &mut Vec<Unit>,
    auras: &crate::model::PlayerAuras,
    combat_beast_bonus_atk: i32,
    next_id: &mut crate::model::UnitId,
    events: &mut Vec<crate::events::Event>,
) {
    for card in hand {
        if card.card_id == ID {
            let copies = if card.is_golden { 2 } else { 1 };
            for _ in 0..copies {
                if board.len() < crate::combat::MAX_BOARD_SIZE {
                    let mut copy = card.clone();
                    copy.id = *next_id;
                    *next_id += 1;
                    crate::cards::sync_unit_auras(&mut copy, auras);
                    if copy.tribe.matches(Tribe::Beast) && combat_beast_bonus_atk != 0 {
                        copy.add_stats(combat_beast_bonus_atk, 0);
                    }
                    copy.sync_max_stats();
                    crate::cards::check_stat_thresholds(&mut copy);
                    events.push(crate::events::Event::UnitSummoned {
                        side,
                        source: copy.id,
                        unit: copy.id,
                        name: copy.name.clone(),
                        attack: copy.attack,
                        health: copy.health,
                        reason: NAME,
                    });
                    board.push(copy);
                }
            }
        }
    }
}
