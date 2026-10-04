//! `Eternal Summoner` (`BG25_009`) — Tier 5 Undead (`8/1`, Reborn).
//!
//! Reborn. Deathrattle: Summon 1 `Eternal Knight` (or a Golden `Eternal Knight` if Golden).

use crate::cards::{tier2, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Keyword, Tribe, Unit};

pub const ID: CardId = 516;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Eternal Summoner", 8, 1, 5)
        .with_tribe(Tribe::Undead)
        .with_keyword(Keyword::Reborn)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let mut knight = tier2::eternal_knight::template().instantiate();
    if dying.is_golden {
        knight.make_golden();
    }
    ctx.summon(dying.id, knight);
}
