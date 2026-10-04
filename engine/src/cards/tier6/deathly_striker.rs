//! `Deathly Striker` (`BG31_835`) — Tier 6 Undead (`8/8`).
//!
//! Avenge (4): Get a (`2` if Golden) random Undead.
//! Deathrattle: Summon it (`them` if Golden) from your hand for this combat only.

use crate::cards::{self, CardTemplate, DeathrattleContext};
use crate::model::{CardId, PlayerAuras, Tribe, Unit};
use crate::rng::Rng;

pub const ID: CardId = 606;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Deathly Striker", 8, 8, 6)
        .with_tribe(Tribe::Undead)
        .on_deathrattle(on_deathrattle)
}

pub fn on_avenge(
    is_golden: bool,
    hero_tier: u32,
    board: &[Unit],
    auras: &mut PlayerAuras,
    hand: &mut Vec<Unit>,
    hand_summoned: &mut Vec<bool>,
    rng: &mut Rng,
) {
    let candidates: Vec<CardTemplate> = cards::full_catalog()
        .into_iter()
        .filter(|t| t.tribe.matches(Tribe::Undead) && t.tavern_tier <= hero_tier.max(1))
        .collect();
    if candidates.is_empty() {
        return;
    }
    let count = if is_golden { 2 } else { 1 };
    for _ in 0..count {
        if hand.len() >= 10 {
            break;
        }
        let pick = rng.below(candidates.len());
        let mut u = candidates[pick].instantiate();
        cards::sync_unit_auras(&mut u, auras);
        hand.push(u);
        hand_summoned.push(false);
        cards::on_card_added_to_hand(board, auras);
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
