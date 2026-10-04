//! `Conveyor Construct` (`BG34_171`) — Tier 4 Mech (`5/2`).
//!
//! Deathrattle: Get a (`2` if Golden) random Magnetic Volumizer(s).

use crate::cards::{self, minions, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;

pub const ID: CardId = 412;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Conveyor Construct", 5, 2, 4)
        .with_tribe(Tribe::Mech)
        .on_deathrattle(on_deathrattle)
}

pub fn draw_random_volumizer(rng: &mut Rng) -> Unit {
    let templates = [
        minions::blue_volumizer::template(),
        minions::green_volumizer::template(),
        minions::red_volumizer::template(),
    ];
    let pick = rng.below(templates.len());
    templates[pick].instantiate()
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let mut vol = draw_random_volumizer(ctx.rng);
        cards::sync_unit_auras(&mut vol, ctx.auras);
        ctx.add_to_hand(vol);
    }
}
