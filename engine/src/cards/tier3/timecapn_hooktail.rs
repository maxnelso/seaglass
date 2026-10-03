//! `Timecap'n Hooktail` (`BG27_005`) — Tier 3 Dragon/Pirate (`1/4`).
//! Whenever you cast a Tavern spell, give your minions `+1` Attack (`+1` Attack twice if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 335;
pub const NAME: &str = "Timecap'n Hooktail";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 4, 3).with_tribe(Tribe::DragonPirate)
}

pub fn on_cast_tavern_spell(state: &mut TavernState) {
    let mut bonus_atk = 0i32;
    for u in &state.board {
        if u.card_id == ID {
            bonus_atk += if u.is_golden { 2 } else { 1 };
        }
    }
    if bonus_atk > 0 {
        for u in &mut state.board {
            u.add_stats(bonus_atk, 0);
        }
    }
}
