//! `Unwilling Slacker` (`BG36_101`) — Tier 3 Aberration (`3/2`).
//! **Deathrattle:** Get a (`two` if Golden) random 1-Cost Tavern spell(s).

use crate::cards::{spells, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 339;
pub const NAME: &str = "Unwilling Slacker";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 2, 3)
        .with_tribe(Tribe::Aberration)
        .on_deathrattle(on_deathrattle)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let spell = spells::draw_random_cost_tavern_spell(1, ctx.rng);
        ctx.add_to_hand(spell);
    }
}
