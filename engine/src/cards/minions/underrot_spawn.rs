//! `Underrot Spawn` (`BG36_116`) — Tier 2 Aberration (`2/2`).
//! **Deathrattle:** Summon a (`2` if Golden) `0/2` Tentacle(s) with **Taunt**.
//! Give your minions `+1` (`+2` if Golden) Attack.

use crate::cards::{tokens, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 232;
pub const NAME: &str = "Underrot Spawn";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 2, 2)
        .with_tribe(Tribe::Aberration)
        .on_deathrattle(on_deathrattle)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        ctx.summon(dying.id, tokens::make_aberrant_tentacle());
    }
    let atk_buff = if dying.is_golden { 2 } else { 1 };
    ctx.buff_all_friendly(atk_buff, 0, "UnderrotSpawn");
}
