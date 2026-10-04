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
