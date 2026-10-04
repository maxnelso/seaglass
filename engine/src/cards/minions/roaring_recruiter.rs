//! `Roaring Recruiter` (`BG29_816`) — Tier 3 Dragon (`2/8`).
//! Whenever another friendly Dragon attacks, give it `+3/+1` (`+6/+2` if Golden).

use crate::cards::{BoardCtx, CardTemplate};
use crate::model::{CardId, Tribe, UnitId};

pub const ID: CardId = 329;
pub const NAME: &str = "Roaring Recruiter";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 8, 3)
        .with_tribe(Tribe::Dragon)
        .on_friendly_attack(on_friendly_attack)
}

pub fn on_friendly_attack(ctx: &mut BoardCtx<'_>, self_idx: usize, attacker_id: UnitId) {
    if ctx.board[self_idx].id == attacker_id {
        return;
    }
    let Some(atk_idx) = ctx.board.iter().position(|u| u.id == attacker_id) else {
        return;
    };
    if !ctx.board[atk_idx].tribe.matches(Tribe::Dragon) {
        return;
    }
    let mult = ctx.board[self_idx].golden_mult();
    ctx.buff_unit(atk_idx, 3 * mult, mult, NAME);
}
