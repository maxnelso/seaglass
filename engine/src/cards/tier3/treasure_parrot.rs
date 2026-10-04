//! `Treasure Parrot` (`BG36_763`) — Tier 3 Beast/Pirate (`5/5`).
//! Once this deals 35 damage, get a (`two` if Golden) `Golden Touch`(es).

use crate::cards::{tokens, BoardCtx, CardTemplate};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 337;
pub const NAME: &str = "Treasure Parrot";
pub const DAMAGE_THRESHOLD: i32 = 35;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 5, 5, 3)
        .with_tribe(Tribe::BeastPirate)
        .on_damage_dealt(on_damage_dealt)
        .on_post_combat(on_post_combat)
}

pub fn on_damage_dealt(ctx: &mut BoardCtx<'_>, self_idx: usize, amount: i32) {
    let unit = &mut ctx.board[self_idx];
    unit.damage_dealt_counter += amount;
    if unit.threshold_triggered || unit.damage_dealt_counter < DAMAGE_THRESHOLD {
        return;
    }
    unit.threshold_triggered = true;
    for _ in 0..unit.golden_mult() {
        ctx.add_to_hand(tokens::make_golden_touch());
    }
}

/// The damage counted in combat carries over to the Tavern copy.
pub fn on_post_combat(pre_combat: &Unit, post_combat_units: &[Unit], tavern_unit: &mut Unit) {
    if let Some(post) = post_combat_units.iter().find(|u| u.id == pre_combat.id) {
        tavern_unit.damage_dealt_counter = post.damage_dealt_counter;
        tavern_unit.threshold_triggered = post.threshold_triggered;
    }
}
