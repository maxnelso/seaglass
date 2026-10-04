//! `Turquoise Skitterer` (`BG31_809`) — Tier 5 Beast (`5/5`).
//!
//! Deathrattle: Your Beetles have `+5/+5` (`+10/+10` if Golden) this game. Summon a (`two` if Golden) `2/2` Beetle(s).

use crate::cards::{tokens, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 552;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Turquoise Skitterer", 5, 5, 5).with_tribe(Tribe::Beast)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let bonus = if dying.is_golden { 10 } else { 5 };
    ctx.auras.beetle_bonus_atk += bonus;
    ctx.auras.beetle_bonus_hp += bonus;
    for idx in 0..ctx.board.len() {
        if ctx.board[idx].card_id == tokens::TOKEN_BEETLE {
            ctx.buff_unit(idx, bonus, bonus, "Turquoise Skitterer");
        }
    }
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let beetle = tokens::make_beetle(false, ctx.auras);
        ctx.summon(dying.id, beetle);
    }
}
