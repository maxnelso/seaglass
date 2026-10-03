//! `Cord Puller` (`BG29_611`) — Tier 1 Mech (`1/1`).
//! **Divine Shield**. **Deathrattle:** Summon a `1/1` (`2/2` if Golden) Microbot.

use crate::cards::{tokens, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Keyword, Tribe, Unit};

pub const ID: CardId = 110;
pub const NAME: &str = "Cord Puller";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 1, 1)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::DivineShield)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    ctx.summon(dying.id, tokens::make_microbot(dying.is_golden));
}
