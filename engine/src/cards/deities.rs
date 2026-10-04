//! Old God Deities: `C'Thun` and `Y'Shaarj` (Patch 36.6.1 / 36.6.3).
//!
//! How summoning a Deity works (`docs/combat.md`, Patch 36.6.3):
//! - Each time a friendly Aberration dies in combat, it advances your Deity's progress by 1.
//! - There are two Deities (`C'Thun` and `Y'Shaarj`), both Tier 1 (`techLevel = 1` in 36.6.3).
//! - Once 4 friendly Aberrations have died in that combat (`DEITY_SACRIFICE_REQUIREMENT = 4`),
//!   your Deity awakens and joins the fight on your board.
//! - `C'Thun`: "Deity. After this awakens, give this minion's stats split amongst your other minions."
//!   (Golden: double this minion's stats split amongst your other minions.)
//! - `Y'Shaarj`: "Deity. Deathrattle: Summon your first 2 Aberrations that died this combat with
//!   their maximum stats (except Deities)." (Golden: first 4 Aberrations.)

use crate::model::{CardId, DeityKind, DeityState, Tribe, Unit};
use crate::rng::Rng;

pub const CARD_CTHUN: CardId = 9901;
pub const CARD_YSHAARJ: CardId = 9902;

/// Number of friendly Aberration deaths required in a single combat to awaken a Deity (Patch 36.6.3).
pub const DEITY_SACRIFICE_REQUIREMENT: u32 = 4;

/// Instantiate the player's Deity unit when it awakens in combat.
pub fn instantiate_deity(deity: &DeityState) -> Unit {
    let (card_id, name) = match deity.kind {
        DeityKind::None | DeityKind::CThun => (CARD_CTHUN, "C'Thun"),
        DeityKind::YShaarj => (CARD_YSHAARJ, "Y'Shaarj"),
    };
    let mut u = Unit::new(name, deity.attack, deity.health)
        .with_card_id(card_id)
        .with_tavern_tier(1)
        .with_tribe(Tribe::Aberration)
        .with_golden(deity.is_golden);
    u.base_attack = if deity.is_golden { 2 } else { 1 };
    u.base_health = if deity.is_golden { 2 } else { 1 };
    u.is_deity = true;
    u
}

/// Resolve `C'Thun`'s awakening effect:
/// "After this awakens, give this minion's stats split amongst your other minions."
/// (Golden: double this minion's stats split amongst your other minions.)
///
/// Each point of Attack and Health is distributed to a uniformly random other living
/// friendly minion on `board`. If there is only 1 other friendly minion, no RNG draws
/// are consumed; if there are 0 other friendly minions, nothing happens.
pub fn on_cthun_awaken(board: &mut [Unit], cthun_idx: usize, rng: &mut Rng) {
    if board.len() <= 1 {
        return;
    }
    let cthun = &board[cthun_idx];
    let mult = if cthun.is_golden { 2 } else { 1 };
    let total_atk = (cthun.attack.max(0)) * mult;
    let total_hp = (cthun.health.max(0)) * mult;

    // Candidate indices of other living minions on the friendly board (ordered left -> right).
    let candidates: Vec<usize> = (0..board.len()).filter(|&i| i != cthun_idx).collect();
    if candidates.is_empty() {
        return;
    }

    for _ in 0..total_atk {
        let pick_idx = if candidates.len() == 1 {
            0
        } else {
            rng.below(candidates.len())
        };
        let target_pos = candidates[pick_idx];
        board[target_pos].add_stats(1, 0);
    }

    for _ in 0..total_hp {
        let pick_idx = if candidates.len() == 1 {
            0
        } else {
            rng.below(candidates.len())
        };
        let target_pos = candidates[pick_idx];
        board[target_pos].add_stats(0, 1);
    }
}

/// Build the resummoned minions for `Y'Shaarj`'s Deathrattle:
/// "Deathrattle: Summon your first 2 Aberrations that died this combat with their maximum stats (except Deities)."
/// (Golden: first 4 Aberrations.)
pub fn yshaarj_deathrattle_summons(is_golden: bool, dead_aberrations: &[Unit]) -> Vec<Unit> {
    let limit = if is_golden { 4 } else { 2 };
    dead_aberrations
        .iter()
        .filter(|u| !u.is_deity)
        .take(limit)
        .map(|orig| {
            let mut copy = orig.clone();
            copy.attack = orig.max_attack.max(orig.base_attack);
            copy.health = orig.max_health.max(1);
            copy.divine_shield = orig.inherent_divine_shield;
            copy
        })
        .collect()
}
