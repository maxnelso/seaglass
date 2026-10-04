//! `Stitched Salvager` (`BG31_999`) — Tier 7 Undead (`16/4`).
//!
//! Start of Combat: Destroy the minion to the left (adjacent minions if Golden).
//! Deathrattle: Summon an exact copy of it (them if Golden). (Except `Stitched Salvager`.)

use crate::cards::{CardTemplate, DeathrattleContext, SocAction};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 710;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Stitched Salvager", 16, 4, 7)
        .with_tribe(Tribe::Undead)
        .on_deathrattle(on_deathrattle)
        .on_start_of_combat_action(|u| SocAction::DestroyNeighbors {
            both_sides: u.is_golden,
        })
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    for stored in &dying.stitched_stored {
        ctx.summon(dying.id, stored.clone());
    }
}
