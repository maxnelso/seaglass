//! `Cage Gnawer` (`BG36_211`) — Tier 4 Beast (`2/7`).
//!
//! Whenever a friendly Beast attacks, give your Beasts `+2/+1` (`+4/+2` if Golden).

use crate::cards::{BoardCtx, CardTemplate};
use crate::model::{CardId, Tribe, UnitId};

pub const ID: CardId = 411;
pub const NAME: &str = "Cage Gnawer";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 7, 4)
        .with_tribe(Tribe::Beast)
        .on_friendly_attack(on_friendly_attack)
}

pub fn on_friendly_attack(ctx: &mut BoardCtx<'_>, self_idx: usize, attacker_id: UnitId) {
    let attacker_is_beast = ctx
        .board
        .iter()
        .any(|u| u.id == attacker_id && u.tribe.matches(Tribe::Beast));
    if !attacker_is_beast {
        return;
    }
    let mult = ctx.board[self_idx].golden_mult();
    for idx in 0..ctx.board.len() {
        if ctx.board[idx].tribe.matches(Tribe::Beast) {
            ctx.buff_unit(idx, 2 * mult, mult, NAME);
        }
    }
}
