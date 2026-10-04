//! `Disguised Graverobber` (`BG28_303`) — Tier 3 Neutral (`4/4`).
//! **Battlecry:** Destroy a friendly Undead to get a (`2` if Golden) plain copy(ies) of it.

use crate::cards::{self, tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
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
    rng: &mut Rng,
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
    let destroyed = state.board[t_idx].clone();
    state.destroy_board_unit(t_idx, pool, rng);

    let copies = if unit.is_golden { 2 } else { 1 };
    let catalog = cards::full_catalog();
    for _ in 0..copies {
        if state.hand.len() >= 10 {
            break;
        }
        let plain = if let Some(tpl) = catalog.iter().find(|t| t.card_id == destroyed.card_id) {
            tpl.instantiate()
        } else if let Some(token) = tokens::make_plain_token(&destroyed, &state.auras) {
            token
        } else {
            let name = destroyed
                .name
                .strip_prefix("Golden ")
                .unwrap_or(&destroyed.name)
                .to_string();
            let divisor = if destroyed.is_golden { 2 } else { 1 };
            let base_atk = destroyed.base_attack / divisor;
            let base_hp = (destroyed.base_health / divisor).max(1);
            let mut copy = Unit::new(name, base_atk, base_hp)
                .with_card_id(destroyed.card_id)
                .with_tavern_tier(destroyed.tavern_tier)
                .with_tribe(destroyed.tribe);
            copy.divine_shield = destroyed.inherent_divine_shield;
            copy.inherent_divine_shield = destroyed.inherent_divine_shield;
            copy
        };
        pool.take_copy(plain.card_id);
        state.add_to_hand(plain);
    }
}

