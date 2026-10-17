//! `Grave Narrator` (`BGDUO_112`) — Tier 4 Undead (`2/7`), Duos only.
//! **Avenge (3):** Your teammate gets a random minion of their most common type (Golden
//! `BGDUO_112_G`, `4/14`: `2` random minions).
//!
//! Combat deaths only, like every Avenge. "Their most common type" is read from the teammate's
//! warband as it entered the combat (`BoardCtx::teammate_board`), as `Friendly Bounty` reads a
//! board (`docs/tavern.md` §7.8); the minions are of that type and of this player's Tavern Tier
//! or lower, from the game's catalog (`cards::game_templates`), and are gifts: they go into the
//! teammate's combat hand at once (`docs/duos.md` §2, `docs/duos_tavern.md` §8.2).

use crate::cards::{self, spells, BoardCtx, CardFlags, CardTemplate};
use crate::model::{CardId, TeammateGift, Tribe, Unit};

pub const ID: CardId = 10402;
pub const NAME: &str = "Grave Narrator";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 7, 4)
        .with_flags(CardFlags::AVENGE)
        .with_tribe(Tribe::Undead)
        .on_friendly_death(on_friendly_death)
}

/// Avenge (3), combat deaths only.
pub fn on_friendly_death(ctx: &mut BoardCtx<'_>, self_idx: usize, _dying: &Unit) {
    if !ctx.in_combat || !ctx.board[self_idx].avenge(3) {
        return;
    }
    let count = ctx.board[self_idx].golden_mult();
    let tribe = spells::most_common_tribe(ctx.teammate_board, ctx.rng);
    let max_tier = ctx.hero_tier.max(1);
    let candidates: Vec<&CardTemplate> = cards::game_templates(ctx.auras)
        .iter()
        .filter(|t| t.tribe.matches(tribe) && t.tavern_tier <= max_tier)
        .collect();
    if candidates.is_empty() {
        return;
    }
    for _ in 0..count {
        let pick = ctx.rng.below(candidates.len());
        ctx.auras
            .give_teammate(TeammateGift::Card(candidates[pick].instantiate()));
    }
}
