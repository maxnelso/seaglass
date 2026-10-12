//! `Geomagus Roogug` (`BG28_583`) — Tier 4 Quilboar (`4/6`, Divine Shield).
//!
//! Divine Shield. Whenever a Blood Gem is played on this, this plays a (`2` if Golden) Blood Gem(s) on a different friendly minion.
//!
//! Each Blood Gem played on it is a trigger of its own, picking its own random minion (a Golden
//! one plays both of a trigger's Blood Gems on that minion). It never plays them on another
//! `Geomagus Roogug`: with no other minion, nothing happens (so two Roogugs don't loop). In the
//! Tavern, each Blood Gem it plays counts as a spell cast on that minion (`Felboar`, `Glambot`,
//! ...; `docs/tavern.md` §7.4).

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

/// Queue one trigger per Blood Gem played on this.
pub fn on_blood_gems_played(unit: &mut Unit, count: u32) {
    unit.pending += count;
}

/// Resolve this unit's queued triggers, one at a time: each plays a (`2` if Golden) Blood Gem(s)
/// on its own random friendly minion other than a `Geomagus Roogug` (nothing if there is none).
pub fn resolve_procs(board: &mut [Unit], self_idx: usize, auras: &PlayerAuras, rng: &mut Rng) {
    let triggers = std::mem::take(&mut board[self_idx].pending);
    let gems = if board[self_idx].is_golden { 2 } else { 1 };
    for _ in 0..triggers {
        let candidates: Vec<usize> = (0..board.len())
            .filter(|&i| board[i].card_id != ID && board[i].health > 0)
            .collect();
        let pick = match candidates.len() {
            0 => return,
            1 => candidates[0],
            n => candidates[rng.below(n)],
        };
        board[pick].play_blood_gems_by_minion(gems, auras);
    }
}
