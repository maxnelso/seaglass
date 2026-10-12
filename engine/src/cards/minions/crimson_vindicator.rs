//! `Crimson Vindicator` (`BG36_241`) — Tier 6 Dragon (`8/9`, Divine Shield).
//!
//! Divine Shield. Rally: Cast `Mighty Dragonbreath` (`twice` if Golden).
//!
//! Each cast gets your spell power: the permanent bonuses plus the spell auras of the friendly
//! minions alive when it is cast (`docs/combat.md` §2).
//!
//! Each cast in combat counts as you casting a Tavern spell (`docs/combat.md` §2): it counts for
//! `Faceless Converter`, and your "whenever you cast a Tavern spell" minions react before it takes
//! effect; it doesn't become the last Tavern spell you cast (`Cataclysmic Harbinger`), and
//! `Felboar` doesn't count it.

use crate::cards::{self, CardTemplate, CombatCastCtx, RallyCtx};
use crate::model::{CardId, Keyword, Tribe, Unit};

pub const ID: CardId = 604;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Crimson Vindicator", 8, 9, 6)
        .with_tribe(Tribe::Dragon)
        .with_keyword(Keyword::DivineShield)
        .on_rally(on_rally)
}

pub fn on_rally(ctx: &mut RallyCtx<'_>) -> Vec<Unit> {
    let casts = if ctx.is_golden { 2 } else { 1 };
    let in_combat = ctx.in_combat;
    let mut cast = ctx.cast_ctx();
    for _ in 0..casts {
        if in_combat {
            cards::cast_tavern_spell_in_combat(&mut cast, mighty_dragonbreath);
        } else {
            // A Rally triggered in the Tavern (`Sky-hatch Runaway`) only gives the stats.
            mighty_dragonbreath(&mut cast);
        }
    }
    Vec::new()
}

/// `Mighty Dragonbreath`, cast in combat: give your minions +3/+2 (plus your spell power), again
/// if they are Dragons, and again if they have Divine Shield.
fn mighty_dragonbreath(ctx: &mut CombatCastCtx<'_>) {
    let (atk, hp) = cards::spell_stat_buff_on_board(ctx.board, ctx.auras, 3, 2);
    for idx in 0..ctx.board.len() {
        let u = &ctx.board[idx];
        if u.health <= 0 {
            continue;
        }
        let mult = 1 + i32::from(u.tribe.matches(Tribe::Dragon)) + i32::from(u.divine_shield);
        ctx.buff_unit(idx, atk * mult, hp * mult, "Crimson Vindicator");
    }
}
