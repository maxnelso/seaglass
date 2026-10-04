//! `Deathly Striker` (`BG31_835`) — Tier 6 Undead (`8/8`).
//!
//! Avenge (4): Get a (`2` if Golden) random Undead.
//! Deathrattle: Summon it (`them` if Golden) from your hand for this combat only.

use crate::cards::{self, BoardCtx, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 606;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Deathly Striker", 8, 8, 6)
        .with_tribe(Tribe::Undead)
        .on_deathrattle(on_deathrattle)
        .on_friendly_death(on_friendly_death)
}

/// Avenge (4), combat deaths only.
pub fn on_friendly_death(ctx: &mut BoardCtx<'_>, self_idx: usize, _dying: &Unit) {
    if !ctx.in_combat || !ctx.board[self_idx].avenge(4) {
        return;
    }
    let is_golden = ctx.board[self_idx].is_golden;
    on_avenge(ctx, is_golden);
}

/// Get a (`2` if Golden) random Undead of the hero's Tavern Tier or lower.
pub fn on_avenge(ctx: &mut BoardCtx<'_>, is_golden: bool) {
    let max_tier = ctx.hero_tier.max(1);
    let candidates: Vec<&CardTemplate> = cards::all_templates()
        .iter()
        .filter(|t| t.tribe.matches(Tribe::Undead) && t.tavern_tier <= max_tier)
        .collect();
    if candidates.is_empty() {
        return;
    }
    let count = if is_golden { 2 } else { 1 };
    for _ in 0..count {
        if ctx.hand.len() >= 10 {
            break;
        }
        let pick = ctx.rng.below(candidates.len());
        ctx.add_to_hand(candidates[pick].instantiate());
    }
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let Some(hand_idx) = (0..ctx.hand.len()).find(|&i| {
            !ctx.hand[i].is_spell
                && ctx.hand[i].tribe.matches(Tribe::Undead)
                && !ctx.hand_summoned.get(i).copied().unwrap_or(false)
        }) else {
            break;
        };
        if hand_idx < ctx.hand_summoned.len() {
            ctx.hand_summoned[hand_idx] = true;
        }
        let summoned = ctx.hand[hand_idx].clone();
        ctx.summon(dying.id, summoned);
    }
}
