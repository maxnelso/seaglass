//! `Al'Akir` (`TB_BaconShop_HERO_76`) — Armor 15 (Duos 13).
//!
//! Hero Power: `Swatting Insects` (`TB_BaconShop_HP_086`), 0 Gold: Start of Combat: Give your left-most minion Windfury, Divine Shield, and Taunt.
//!
//! Passive. At Start of Combat (`docs/heroes.md` §7.2) the left-most minion gains the three
//! keywords; nothing happens without a minion.

use crate::cards::{CardHooks, HeroCombatCtx};
use crate::model::{CardId, HeroId, Keyword};

pub const ID: HeroId = 64403;
pub const POWER: CardId = 64402;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY.on_hero_start_of_combat(start_of_combat),
    )]
}

/// Start of Combat: the left-most minion gains Windfury, Divine Shield and Taunt.
fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    if let Some(unit) = ctx.own.board.first_mut() {
        unit.apply_keyword(Keyword::Windfury, false);
        unit.apply_keyword(Keyword::DivineShield, false);
        unit.apply_keyword(Keyword::Taunt, false);
    }
}
