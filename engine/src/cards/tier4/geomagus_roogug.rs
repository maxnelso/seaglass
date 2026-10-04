//! `Geomagus Roogug` (`BG28_583`) — Tier 4 Quilboar (`4/6`, Divine Shield).
//!
//! Divine Shield. Whenever a Blood Gem is played on this, this plays a (`2` if Golden) Blood Gem(s) on a different friendly minion.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, PlayerAuras, Tribe, Unit};
use crate::rng::Rng;

pub const ID: CardId = 423;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Geomagus Roogug", 4, 6, 4)
        .with_tribe(Tribe::Quilboar)
        .with_keyword(Keyword::DivineShield)
        .on_blood_gems_played(on_blood_gems_played)
        .on_resolve_pending(resolve_procs)
}

pub fn on_blood_gems_played(unit: &mut Unit, count: u32) {
    let mult = if unit.is_golden { 2 } else { 1 };
    unit.pending_roogug_gems += count * mult;
}

/// Play this unit's queued Blood Gem procs on a different random friendly minion (preferring
/// non-Roogug minions). Procs never chain: the recipient's own queue is left untouched.
pub fn resolve_procs(board: &mut [Unit], self_idx: usize, auras: &PlayerAuras, rng: &mut Rng) {
    let gem_count = std::mem::take(&mut board[self_idx].pending_roogug_gems);
    if gem_count == 0 || board.len() <= 1 {
        return;
    }
    let candidates: Vec<usize> = (0..board.len())
        .filter(|&i| i != self_idx && board[i].card_id != ID)
        .collect();
    let pool: Vec<usize> = if candidates.is_empty() {
        (0..board.len()).filter(|&i| i != self_idx).collect()
    } else {
        candidates
    };
    let pick = if pool.len() == 1 {
        pool[0]
    } else {
        pool[rng.below(pool.len())]
    };
    let queued = board[pick].pending_roogug_gems;
    board[pick].play_blood_gems(gem_count, auras);
    board[pick].pending_roogug_gems = queued;
}
