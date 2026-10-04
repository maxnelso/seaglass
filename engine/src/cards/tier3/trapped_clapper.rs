//! `Trapped Clapper` (`BG36_730`) — Tier 3 Demon (`2/2`).
//! **Deathrattle:** Add a (`two` if Golden) **Fodder**(s) to your next 3 **Refreshes**.

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 336;
pub const NAME: &str = "Trapped Clapper";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 2, 3)
        .with_tribe(Tribe::Demon)
        .on_deathrattle(on_deathrattle)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for slot in &mut ctx.auras.fodder_per_refresh {
        *slot += count;
    }
}
