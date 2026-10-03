//! `Harmless Bonehead` (`BG28_300`) — Tier 1 Undead (`1/1`).
//! **Deathrattle:** Summon two `1/1` (`2/2` if Golden) Skeletons.

use crate::cards::{tokens, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 118;
pub const NAME: &str = "Harmless Bonehead";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 1, 1).with_tribe(Tribe::Undead)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    for _ in 0..2 {
        let skel = tokens::make_skeleton(dying.is_golden, ctx.auras);
        ctx.summon(dying.id, skel);
    }
}
