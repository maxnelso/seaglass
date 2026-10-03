//! `Drifting Sacrifice` (`BG36_113`) — Tier 3 Aberration (`2/1`).
//! **Reborn**. **Deathrattle:** Give your **Deity** `+2/+1` (`+4/+2` if Golden).

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, Keyword, Tribe, Unit};

pub const ID: CardId = 313;
pub const NAME: &str = "Drifting Sacrifice";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 1, 3)
        .with_tribe(Tribe::Aberration)
        .with_keyword(Keyword::Reborn)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let mult = if dying.is_golden { 2 } else { 1 };
    let d_atk = 2 * mult;
    let d_hp = mult;
    ctx.auras.deity.attack += d_atk;
    ctx.auras.deity.health += d_hp;
    for idx in 0..ctx.board.len() {
        if ctx.board[idx].is_deity {
            ctx.buff_unit(idx, d_atk, d_hp, NAME);
        }
    }
}
