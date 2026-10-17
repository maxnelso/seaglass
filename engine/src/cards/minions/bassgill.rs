//! `Bassgill` (`BG26_350`) — Tier 3 Murloc (`5/2`), in no pool.
//! **Deathrattle:** Summon the highest-Health (`2 highest-Health` if Golden) Murloc(s) from your
//! hand for this combat only.
//!
//! Only `Bassgill Portrait` gives it (`docs/trinkets.md` §11.9). Each summon is a copy of the
//! highest-Health Murloc in hand not yet summoned from hand this combat (picked at random among
//! those tied for it), which locks it as `Diremuck Forager`'s do (`docs/combat.md` §2); with no
//! room or no such Murloc, nothing.

use crate::cards::{BoardCtx, CardTemplate};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 10802;
pub const NAME: &str = "Bassgill";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 5, 2, 3)
        .with_tribe(Tribe::Murloc)
        .on_deathrattle(deathrattle)
}

pub fn deathrattle(dying: &Unit, ctx: &mut BoardCtx<'_>) {
    for _ in 0..dying.golden_mult() {
        let murlocs: Vec<usize> = (0..ctx.hand.len())
            .filter(|&i| {
                let card = &ctx.hand[i];
                !ctx.hand_summoned.get(i).copied().unwrap_or(true)
                    && !card.is_spell
                    && card.tribe.matches(Tribe::Murloc)
            })
            .collect();
        let Some(best) = murlocs.iter().map(|&i| ctx.hand[i].health).max() else {
            return;
        };
        let tied: Vec<usize> = murlocs
            .into_iter()
            .filter(|&i| ctx.hand[i].health == best)
            .collect();
        let hand_idx = if tied.len() == 1 {
            tied[0]
        } else {
            tied[ctx.rng.below(tied.len())]
        };
        let copy = ctx.hand[hand_idx].clone();
        if ctx.summon_as(dying.id, copy, NAME).is_none() {
            return;
        }
        ctx.hand_summoned[hand_idx] = true;
    }
}
