//! `Forest Rover` (`BG31_801`) — Tier 2 Beast (`1/1`).
//! **Battlecry:** Your Beetles have `+2/+1` (`+4/+2` if Golden) this game.
//! **Deathrattle:** Summon a (`two` if Golden) `2/2` Beetle(s).

use crate::cards::{tokens, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 212;
pub const NAME: &str = "Forest Rover";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 1, 2)
        .with_tribe(Tribe::Beast)
        .on_battlecry(|state, unit, _, _, _| on_battlecry(state, unit))
        .on_deathrattle(on_deathrattle)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let mult = if unit.is_golden { 2 } else { 1 };
    let d_atk = 2 * mult;
    let d_hp = mult;
    state.auras.beetle_bonus_atk += d_atk;
    state.auras.beetle_bonus_hp += d_hp;
    for u in &mut state.board {
        if u.card_id == tokens::TOKEN_BEETLE {
            u.add_stats(d_atk, d_hp);
        }
    }
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let beetle = tokens::make_beetle(false, ctx.auras);
        ctx.summon(dying.id, beetle);
    }
}
