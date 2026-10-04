//! `Ghastcoiler` (`BGS_008`) — Tier 5 Beast (`7/7`).
//!
//! Deathrattle: Summon 2 (`4` if Golden) random Deathrattle minions.

use crate::cards::{full_catalog, is_deathrattle_minion, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 523;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Ghastcoiler", 7, 7, 5)
        .with_tribe(Tribe::Beast)
        .on_deathrattle(on_deathrattle)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let pool: Vec<CardTemplate> = full_catalog()
        .into_iter()
        .filter(|t| t.tavern_tier <= 6 && is_deathrattle_minion(t.card_id) && t.card_id != ID)
        .collect();
    if pool.is_empty() {
        return;
    }
    let count = if dying.is_golden { 4 } else { 2 };
    for _ in 0..count {
        let pick = ctx.rng.below(pool.len());
        let summoned = pool[pick].instantiate();
        ctx.summon(dying.id, summoned);
    }
}
