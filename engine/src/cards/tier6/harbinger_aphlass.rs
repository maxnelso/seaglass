//! `Harbinger Aph'lass` (`BGFYM_005`) — Tier 6 Aberration (`3/6`).
//!
//! Whenever you discard a card, give your Deity `+2/+1` (`+4/+2` if Golden) and improve this.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 613;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Harbinger Aph'lass", 3, 6, 6).with_tribe(Tribe::Aberration)
}

pub fn on_discard(state: &mut TavernState) {
    let mut d_atk = 0i32;
    let mut d_hp = 0i32;
    for u in &mut state.board {
        if u.card_id == ID {
            let mult = (1 + u.aphlass_stacks as i32) * (if u.is_golden { 2 } else { 1 });
            d_atk += 2 * mult;
            d_hp += mult;
            u.aphlass_stacks += 1;
        }
    }
    if d_atk > 0 || d_hp > 0 {
        state.auras.deity.attack += d_atk;
        state.auras.deity.health += d_hp;
    }
}
