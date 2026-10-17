//! `Charisma` (`BG36_MidGameEffect_000t3`) — turns 7–11, weight 100.
//!
//! Rally: Get a random minion of your most common type.
//!
//! Rally: as `Dark Paradox`'s, a random minion of Tier 6 or lower of the board's most common type
//! (`spells::most_common_tribe`) from the game's catalog, to the hand (the combat hand in combat).
//! Not on Taunt or Avenge minions, nor on Tier 7 minions (`docs/dark_gifts.md` §5.2).

use crate::cards::{self, spells, CardHooks, CardTemplate, RallyCtx};
use crate::dark_gifts::OfferCtx;
use crate::model::{CardId, Unit};

pub const ID: CardId = 132201;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_gift_rally(rally)
            .on_gift_offerable(offerable),
    )]
}

fn rally(ctx: &mut RallyCtx<'_>) -> Vec<Unit> {
    let tribe = spells::most_common_tribe(ctx.board, ctx.rng);
    let candidates: Vec<&CardTemplate> = cards::game_templates(ctx.auras)
        .iter()
        .filter(|t| t.tavern_tier <= 6 && t.tribe.matches(tribe))
        .collect();
    if !candidates.is_empty() {
        let mut drawn = candidates[ctx.rng.below(candidates.len())].instantiate();
        cards::sync_unit_auras(&mut drawn, ctx.auras);
        ctx.generated_hand.push(drawn);
    }
    Vec::new()
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    !t.taunt && !super::is_avenge(t) && t.tavern_tier != 7
}
