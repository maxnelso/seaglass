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
}

pub fn on_blood_gems_played(unit: &mut Unit, count: u32) {
    if unit.card_id == ID {
        let mult = if unit.is_golden { 2 } else { 1 };
        unit.pending_roogug_gems += count * mult;
    }
}

pub fn resolve_procs(board: &mut [Unit], auras: &PlayerAuras, rng: &mut Rng) {
    if board.len() <= 1 {
        for u in board.iter_mut() {
            u.pending_roogug_gems = 0;
        }
        return;
    }
    let procs: Vec<(usize, u32)> = board
        .iter_mut()
        .enumerate()
        .filter_map(|(i, u)| {
            if u.pending_roogug_gems > 0 {
                let p = u.pending_roogug_gems;
                u.pending_roogug_gems = 0;
                Some((i, p))
            } else {
                None
            }
        })
        .collect();
    for (src_idx, gem_count) in procs {
        let candidates: Vec<usize> = (0..board.len())
            .filter(|&i| i != src_idx && board[i].card_id != ID)
            .collect();
        let pool: Vec<usize> = if candidates.is_empty() {
            (0..board.len()).filter(|&i| i != src_idx).collect()
        } else {
            candidates
        };
        if !pool.is_empty() {
            let pick = if pool.len() == 1 {
                pool[0]
            } else {
                pool[rng.below(pool.len())]
            };
            board[pick].play_blood_gems(gem_count, auras);
            board[pick].pending_roogug_gems = 0;
        }
    }
}
