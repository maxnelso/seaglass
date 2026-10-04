//! `Goldrinn, the Great Wolf` (`BGS_018`) — Tier 5 Beast (`7/7`).
//!
//! Deathrattle: Your Beasts have `+7/+7` (`+14/+14` if Golden) until next turn.

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 524;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Goldrinn, the Great Wolf", 7, 7, 5).with_tribe(Tribe::Beast)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let amount = if dying.is_golden { 14 } else { 7 };
    ctx.auras.goldrinn_bonus += amount;
    for idx in 0..ctx.board.len() {
        if ctx.board[idx].tribe.matches(Tribe::Beast) {
            ctx.buff_unit(idx, amount, amount, "Goldrinn, the Great Wolf");
        }
    }
}
