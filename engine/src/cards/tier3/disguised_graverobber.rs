//! `Disguised Graverobber` (`BG28_303`) — Tier 3 Neutral (`4/4`).
//! **Battlecry:** Destroy a friendly Undead to get a (`2` if Golden) plain copy(ies) of it.

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 312;
pub const NAME: &str = "Disguised Graverobber";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 4, 3).with_tribe(Tribe::None)
}

pub fn on_battlecry(
    state: &mut TavernState,
    unit: &Unit,
    board_pos: usize,
    pool: &mut CardPool,
) {
    // Prefer the minion at `board_pos` (the slot index specified by `Play`), otherwise the nearest friendly Undead.
    let target_idx = if board_pos < state.board.len()
        && state.board[board_pos].tribe.matches(Tribe::Undead)
    {
        Some(board_pos)
    } else if board_pos > 0
        && board_pos - 1 < state.board.len()
        && state.board[board_pos - 1].tribe.matches(Tribe::Undead)
    {
        Some(board_pos - 1)
    } else {
        state
            .board
            .iter()
            .position(|u| u.tribe.matches(Tribe::Undead))
    };

    let Some(t_idx) = target_idx else {
        return;
    };
    let destroyed = state.board.remove(t_idx);
    pool.return_unit(&destroyed);
    if destroyed.card_id == cards::tier2::eternal_knight::ID {
        state.auras.eternal_knights_died += 1;
        state.sync_all_auras();
    }

    let copies = if unit.is_golden { 2 } else { 1 };
    let catalog = cards::full_catalog();
    for _ in 0..copies {
        if state.hand.len() >= 10 {
            break;
        }
        let mut plain = if let Some(tpl) = catalog.iter().find(|t| t.card_id == destroyed.card_id) {
            tpl.instantiate()
        } else {
            let mut copy = Unit::new(
                destroyed.name.clone(),
                destroyed.base_attack,
                destroyed.base_health,
            )
            .with_card_id(destroyed.card_id)
            .with_tavern_tier(destroyed.tavern_tier)
            .with_tribe(destroyed.tribe);
            copy.taunt = destroyed.taunt;
            copy.divine_shield = destroyed.inherent_divine_shield;
            copy.inherent_divine_shield = destroyed.inherent_divine_shield;
            copy.windfury = destroyed.windfury;
            copy.reborn = destroyed.reborn;
            copy.venomous = destroyed.venomous;
            copy.stealth = destroyed.stealth;
            copy.magnetic = destroyed.magnetic;
            copy
        };
        state.apply_global_unit_auras(&mut plain);
        state.add_to_hand(plain);
    }
}
