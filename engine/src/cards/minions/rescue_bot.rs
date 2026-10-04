//! `Rescue Bot` (`BG36_854`) — Tier 3 Mech (`2/1`).
//! **Taunt**. **Deathrattle:** Get a (`2` if Golden) `Repair Job`(s).

use crate::cards::{spells, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Keyword, Tribe, Unit};

pub const ID: CardId = 328;
pub const NAME: &str = "Rescue Bot";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 1, 3)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::Taunt)
        .on_deathrattle(on_deathrattle)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        ctx.add_to_hand(spells::make_repair_job());
    }
}
