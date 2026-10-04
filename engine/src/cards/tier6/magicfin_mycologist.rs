//! `Magicfin Mycologist` (`BG33_891`) — Tier 6 Murloc (`4/8`).
//!
//! Once per turn (`Twice` if Golden), after you buy a Tavern spell, get a `1/1` Murloc and teach it that spell.

use crate::cards::{spells, tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 616;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Magicfin Mycologist", 4, 8, 6)
        .with_tribe(Tribe::Murloc)
        .on_reset_turn_charges(reset_turn_charges)
        .on_made_golden(|u| u.mycologist_charges_left += 1)
}

pub fn reset_turn_charges(unit: &mut Unit) {
    unit.mycologist_charges_left = if unit.is_golden { 2 } else { 1 };
}

pub fn after_buy_spell(state: &mut TavernState, spell_id: CardId) {
    if !spells::is_tavern_spell(spell_id) {
        return;
    }
    let mut triggers = 0u32;
    for u in &mut state.board {
        if u.card_id == ID && u.mycologist_charges_left > 0 {
            u.mycologist_charges_left -= 1;
            triggers += 1;
        }
    }
    for _ in 0..triggers {
        state.add_to_hand(tokens::make_magicfin_apprentice(spell_id));
    }
}

pub fn on_apprentice_battlecry(
    state: &mut TavernState,
    unit: &Unit,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let Some(spell_id) = unit.taught_spell_id else {
        return;
    };
    if let Some(spell) = spells::spell_by_id(spell_id) {
        let target = if spells::spell_requires_board_target(spell_id) && !state.board.is_empty() {
            board_pos.min(state.board.len() - 1)
        } else {
            0
        };
        spells::cast_spell(state, spell, target, pool, rng);
    }
}
