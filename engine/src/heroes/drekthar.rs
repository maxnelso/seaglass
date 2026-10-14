//! `Drek'Thar` (`BG22_HERO_002`) — Armor 10 (Duos 8).
//!
//! Hero Power: `Frostwolf Fervor` (`BG22_HERO_002p`), 0 Gold: When you have space in combat, summon a copy of your highest-Attack minion. (Unlocks on Turn 7.)
//!
//! Passive. It counts the turns (`counter`); from the 7th on, each combat is armed. Once per
//! combat (engine decision), as soon as there is room on the board (`hero_combat_space`: at Start
//! of Combat, or after a round of deaths), a copy of the friendly minion with the most Attack (a
//! tie at random) is summoned at the right end: its current stats, keywords and state.

use crate::cards::{BoardCtx, CardHooks};
use crate::heroes::{count_turn_and_arm, summon_copy_of_best};
use crate::model::{CardId, HeroId};

pub const ID: HeroId = 80239;
pub const POWER: CardId = 80244;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_turn_reset(count_turn_and_arm)
            .on_hero_combat_space(summon_copy),
    )]
}

/// Room opened: a copy of the highest-Attack minion, once per combat.
fn summon_copy(ctx: &mut BoardCtx<'_>) {
    summon_copy_of_best(ctx, |u| u.attack, "Frostwolf Fervor");
}
