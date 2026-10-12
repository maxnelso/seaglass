//! `Deathly Striker` (`BG31_835`) — Tier 6 Undead (`8/8`).
//!
//! Avenge (4): Get a (`2` if Golden) random Undead.
//! Deathrattle: Summon it (`them` if Golden) from your hand for this combat only.
//!
//! The Deathrattle summons exactly the Undead this minion's Avenge gave you this combat (all of
//! them if it triggered more than once; nothing if it never did), not other Undead in hand. Each
//! is summoned once: an extra trigger (`Titus Rivendare`) summons nothing more. Its Avenge never
//! gets a `Deathly Striker` (Golden or not).

use crate::cards::{self, BoardCtx, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit, UnitId};

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
    let (self_id, is_golden) = (ctx.board[self_idx].id, ctx.board[self_idx].is_golden);
    on_avenge(ctx, self_id, is_golden);
}

/// Get a (`2` if Golden) random Undead other than a Deathly Striker, of the hero's Tavern Tier or
/// lower, recorded as given by the Striker `self_id` for its Deathrattle.
pub fn on_avenge(ctx: &mut BoardCtx<'_>, self_id: UnitId, is_golden: bool) {
    let max_tier = ctx.hero_tier.max(1);
    let candidates: Vec<&CardTemplate> = cards::all_templates()
        .iter()
        .filter(|t| t.tribe.matches(Tribe::Undead) && t.tavern_tier <= max_tier && t.card_id != ID)
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
        ctx.add_to_hand_from(self_id, candidates[pick].instantiate());
    }
}

/// Summon, from hand for this combat only, every Undead this Striker's Avenge gave this combat.
pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    for hand_idx in 0..ctx.hand.len() {
        let given = ctx.hand_given_by.get(hand_idx) == Some(&Some(dying.id));
        if !given || ctx.hand_summoned.get(hand_idx).copied().unwrap_or(true) {
            continue;
        }
        let summoned = ctx.hand[hand_idx].clone();
        if ctx.summon_as(dying.id, summoned, "Deathrattle").is_some() {
            ctx.hand_summoned[hand_idx] = true;
        }
    }
}
