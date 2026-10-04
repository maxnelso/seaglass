//! `Imp-lusionist` (`BG36_731`) — Tier 4 Demon (`4/2`).
//!
//! Deathrattle: Get a (`2` if Golden) Methodical Madness.

use crate::cards::{spells, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 434;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Imp-lusionist", 4, 2, 4).with_tribe(Tribe::Demon)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        ctx.add_to_hand(spells::make_methodical_madness());
    }
}
