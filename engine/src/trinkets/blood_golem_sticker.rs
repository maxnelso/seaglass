//! `Blood Golem Sticker` (`BG30_MagicItem_442`) — Greater, 5 Gold, Quilboar.
//!
//! Whenever a friendly Quilboar dies, summon a Golem with stats equal to its Blood Gems.
//!
//! `trinket_friendly_death`: a friendly Quilboar that dies in combat summons a `Blood Golem` with
//! the stats its Blood Gems gave it (`Unit::blood_gem_stats_applied`) at the right end, if there
//! is room. A Quilboar without Blood Gem stats summons nothing (**engine decision**).

use crate::cards::{tokens, BoardCtx, CardHooks};
use crate::heroes::HERO_SOURCE;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 113096;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_friendly_death(friendly_death),
    )]
}

fn friendly_death(ctx: &mut BoardCtx<'_>, dying: &Unit) {
    if !dying.tribe.matches(Tribe::Quilboar) {
        return;
    }
    let (attack, health) = dying.blood_gem_stats_applied;
    if attack <= 0 && health <= 0 {
        return;
    }
    let golem = tokens::make_blood_golem(attack.max(0), health.max(1), false);
    ctx.cursor = ctx.board.len();
    ctx.summon_as(HERO_SOURCE, golem, "Blood Golem Sticker");
}
