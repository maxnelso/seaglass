//! `Cadaver Caretaker` (`BG30_125`) — Tier 3 Undead (`3/3`).
//! **Deathrattle:** Summon three (`six` if Golden) `1/1` Skeletons.

use crate::cards::{tokens, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 308;
pub const NAME: &str = "Cadaver Caretaker";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 3)
        .with_tribe(Tribe::Undead)
        .on_deathrattle(on_deathrattle)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 6 } else { 3 };
    for _ in 0..count {
        let skel = tokens::make_skeleton(false, ctx.auras);
        ctx.summon(dying.id, skel);
    }
}
