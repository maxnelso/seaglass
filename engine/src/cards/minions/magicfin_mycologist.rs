//! `Magicfin Mycologist` (`BG33_891`) — Tier 6 Murloc (`4/8`).
//!
//! Once per turn (`Twice` if Golden), after you buy a Tavern spell, get a `1/1` Murloc and teach it that spell.
//!
//! The Murloc, a `Magicfin Apprentice`, has "Battlecry: Cast the taught spell". A spell cast on a
//! minion makes it a targeted Battlecry (`docs/tavern.md` §7.1): the target is chosen with the
//! play position among the friendly minions the spell can target (none: nothing is cast), and a
//! repeat (`Brann Bronzebeard`) keeps it, doing nothing once it is gone or transformed.

use crate::cards::{self, spells, tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 616;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Magicfin Mycologist", 4, 8, 6)
        .with_tribe(Tribe::Murloc)
        .on_reset_turn_charges(reset_turn_charges)
        .on_made_golden(|u| u.charges += 1)
        .on_after_buy(after_buy)
}

pub fn reset_turn_charges(unit: &mut Unit) {
    unit.charges = if unit.is_golden { 2 } else { 1 };
}

pub fn after_buy(
    state: &mut TavernState,
    self_idx: usize,
    bought: &Unit,
    _: &mut CardPool,
    _: &mut Rng,
) {
    if !bought.is_spell || !spells::is_tavern_spell(bought.card_id) {
        return;
    }
    let mycologist = &mut state.board[self_idx];
    if mycologist.charges == 0 {
        return;
    }
    mycologist.charges -= 1;
    state.add_to_hand(tokens::make_magicfin_apprentice(bought.card_id));
}

pub fn on_apprentice_battlecry(
    state: &mut TavernState,
    unit: &Unit,
    _: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let Some(spell) = unit.taught_spell_id.and_then(spells::spell_by_id) else {
        return;
    };
    if !spells::spell_requires_board_target(spell.card_id) {
        spells::cast_spell(state, spell, 0, pool, rng);
    } else if let Some(target) = cards::battlecry_target_pos(&state.board) {
        spells::cast_spell(state, spell, target, pool, rng);
    }
}

/// The target of the Apprentice's Battlecry if its spell is cast on a minion: a friendly minion
/// the spell can target ([`spells::can_target`]) at `board_pos` (where the Apprentice is played),
/// else the one to its left, else the first one (`None`: no valid target, or an untargeted spell).
pub fn pick_apprentice_target(state: &TavernState, unit: &Unit, board_pos: usize) -> Option<usize> {
    let spell_id = unit.taught_spell_id?;
    if !spells::spell_requires_board_target(spell_id) {
        return None;
    }
    let valid = |u: &Unit| spells::can_target(spell_id, u);
    let valid_at = |pos: usize| state.board.get(pos).is_some_and(valid);
    if valid_at(board_pos) {
        Some(board_pos)
    } else if board_pos > 0 && valid_at(board_pos - 1) {
        Some(board_pos - 1)
    } else {
        state.board.iter().position(valid)
    }
}
