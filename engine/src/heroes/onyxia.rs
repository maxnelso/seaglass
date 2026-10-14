//! `Onyxia` (`BG22_HERO_305`) — Armor 10 (Duos 8).
//!
//! Hero Power: `Broodmother` (`BG22_HERO_305p`), 0 Gold: Avenge (4): Summon a 1/1 Whelp that attacks immediately. Improve this by +1/+1.
//!
//! Passive. Each friendly minion that dies in combat counts (`hero_friendly_death`, `counter`;
//! the count starts again each combat: engine decision); every 4th summons an `Onyxian Whelp`
//! (`BG22_HERO_305t`, a Dragon) of `1 + stacks` / `1 + stacks` at the right end, which attacks
//! immediately (it doesn't move the attack pointer), and adds a stack: the improvement lasts the
//! game.

use crate::cards::{BoardCtx, CardHooks};
use crate::heroes::{power_mut, HERO_SOURCE};
use crate::model::{CardId, HeroId, HeroState, Tribe, Unit};

pub const ID: HeroId = 82110;
pub const POWER: CardId = 82114;

/// `Onyxian Whelp` (`BG22_HERO_305t`).
pub const WHELP: CardId = 82117;

/// Friendly deaths per Whelp.
const AVENGE: i32 = 4;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_turn_reset(new_turn)
            .on_hero_friendly_death(avenge),
    )]
}

/// Start of turn: the Avenge count starts again for the coming combat.
fn new_turn(hero: &mut HeroState) {
    hero.power.counter = 0;
}

/// A friendly minion died: every 4th, a Whelp that attacks immediately.
fn avenge(ctx: &mut BoardCtx<'_>, _: &Unit) {
    let Some(power) = power_mut(ctx.auras) else {
        return;
    };
    power.counter += 1;
    if power.counter < AVENGE {
        return;
    }
    power.counter -= AVENGE;
    let size = 1 + power.stacks;
    power.stacks += 1;
    let whelp = Unit::new("Onyxian Whelp", size, size)
        .with_card_id(WHELP)
        .with_tribe(Tribe::Dragon);
    ctx.cursor = ctx.board.len();
    if let Some(pos) = ctx.summon_as(HERO_SOURCE, whelp, "Broodmother") {
        let id = ctx.board[pos].id;
        ctx.pending_attacks.push(id);
    }
}
