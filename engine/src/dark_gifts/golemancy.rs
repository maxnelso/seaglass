//! `Golemancy` (`BG36_MidGameEffect_000t61`) — turns 9–11, weight 100.
//!
//! Deathrattle: Summon a Golem with this minion's stats.
//!
//! Deathrattle: summon a `Deserted Golem` (`BG29_864t`, the token a Power.log shows) with its
//! Attack and maximum Health as it died, in its place. Not on minions with no type, nor on
//! `Stitched Salvager` (`docs/dark_gifts.md` §5.2).

use crate::cards::{tokens, BoardCtx, CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::{CardId, DarkGift, Unit};

pub const ID: CardId = 132835;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_gift_deathrattle(deathrattle)
            .on_gift_offerable(offerable),
    )]
}

fn deathrattle(dying: &Unit, _: &DarkGift, ctx: &mut BoardCtx<'_>) {
    let golem = tokens::make_deserted_golem(dying.attack.max(0), dying.max_health.max(1));
    ctx.summon_as(dying.id, golem, "Golemancy");
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    super::has_type(t) && t.name != "Stitched Salvager"
}
