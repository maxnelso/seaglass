//! `Devout Hellcaller` (`BG33_155`) — Tier 3 Demon (`4/4`).
//! After another friendly Demon deals damage, gain `+2/+2` (`+4/+4` if Golden) permanently.

use crate::cards::{BoardCtx, CardTemplate};
use crate::model::{CardId, Tribe, UnitId};

pub const ID: CardId = 310;
pub const NAME: &str = "Devout Hellcaller";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 4, 3)
        .with_tribe(Tribe::Demon)
        .on_after_friendly_damage_dealt(after_friendly_damage_dealt)
}

pub fn after_friendly_damage_dealt(
    ctx: &mut BoardCtx<'_>,
    self_idx: usize,
    _source_id: UnitId,
    source_tribe: Tribe,
) {
    if !source_tribe.matches(Tribe::Demon) {
        return;
    }
    let unit = &mut ctx.board[self_idx];
    let delta = 2 * unit.golden_mult();
    unit.perm_atk_gained += delta;
    unit.perm_hp_gained += delta;
    ctx.buff_unit(self_idx, delta, delta, NAME);
}
