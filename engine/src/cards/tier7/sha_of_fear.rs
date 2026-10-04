//! `Sha of Fear` (`BG36_111`) — Tier 7 Aberration (`10/13`).
//!
//! Whenever you cast a Tavern spell, give your minions and Deity `+3/+3` (`+6/+6` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 708;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Sha of Fear", 10, 13, 7).with_tribe(Tribe::Aberration)
}

pub fn on_cast_tavern_spell(state: &mut TavernState) {
    let mut buff = 0i32;
    for u in &state.board {
        if u.card_id == ID {
            buff += if u.is_golden { 6 } else { 3 };
        }
    }
    if buff == 0 {
        return;
    }
    for u in &mut state.board {
        u.add_stats(buff, buff);
    }
    state.auras.deity.attack += buff;
    state.auras.deity.health += buff;
}
