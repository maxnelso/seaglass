//! `Murcules` (`BG27_023`) — Tier 2 Murloc (`5/3`).
//! Whenever this kills a minion, give a minion in your hand `+3/+3` (`+6/+6` if Golden).
//!
//! Triggers whenever `Murcules`'s combat damage kills an enemy minion (whether `Murcules` attacked
//! or was attacked, and even if `Murcules` also dies in the exchange), before deaths resolve. A
//! random playable minion in the combat hand gains `+3/+3` (`+6/+6` if Golden), kept after combat.

use crate::cards::{BoardCtx, CardTemplate};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 235;
pub const NAME: &str = "Murcules";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 5, 3, 2)
        .with_tribe(Tribe::Murloc)
        .on_kill(on_kill)
}

pub fn on_kill(ctx: &mut BoardCtx<'_>, self_idx: usize, _: &Unit) {
    let candidates: Vec<usize> = ctx
        .hand
        .iter()
        .enumerate()
        .filter(|(_, u)| !u.is_spell && !u.unplayable && u.lockbox_turns_left == 0)
        .map(|(i, _)| i)
        .collect();
    if candidates.is_empty() {
        return;
    }
    let idx = if candidates.len() == 1 {
        candidates[0]
    } else {
        candidates[ctx.rng.below(candidates.len())]
    };
    let buff = 3 * ctx.board[self_idx].golden_mult();
    ctx.hand[idx].add_stats(buff, buff);
}
