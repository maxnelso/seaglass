//! `Vicious Mindslasher` (`BG36_108`) — Tier 3 Aberration (`1/2`).
//! Whenever you cast a Tavern spell, give this and your **Deity** `+1/+2` (`+2/+4` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 340;
pub const NAME: &str = "Vicious Mindslasher";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 2, 3).with_tribe(Tribe::Aberration)
}

pub fn on_cast_tavern_spell(state: &mut TavernState) {
    let mut deity_atk = 0i32;
    let mut deity_hp = 0i32;
    for u in &mut state.board {
        if u.card_id == ID {
            let m = if u.is_golden { 2 } else { 1 };
            u.add_stats(m, 2 * m);
            deity_atk += m;
            deity_hp += 2 * m;
        }
    }
    if deity_atk > 0 || deity_hp > 0 {
        state.auras.deity.attack += deity_atk;
        state.auras.deity.health += deity_hp;
    }
}
