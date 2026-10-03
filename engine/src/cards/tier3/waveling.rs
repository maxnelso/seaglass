//! `Waveling` (`BG34_856`) — Tier 3 Elemental (`5/1`).
//! **Deathrattle:** After the Tavern is **Refreshed** this game, give a random minion in it `+4/+4` (`+4/+4` twice if Golden).

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 341;
pub const NAME: &str = "Waveling";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 5, 1, 3).with_tribe(Tribe::Elemental)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let stacks = if dying.is_golden { 2 } else { 1 };
    ctx.auras.waveling_stacks += stacks;
}
