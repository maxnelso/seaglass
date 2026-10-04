//! `Goldrinn, the Great Wolf` (`BGS_018`) — Tier 5 Beast (`7/7`).
//!
//! Deathrattle: Your Beasts have `+7/+7` (`+14/+14` if Golden) until next turn.

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, EffectDuration, PlayerEffect, Tribe, Unit};

pub const ID: CardId = 524;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Goldrinn, the Great Wolf", 7, 7, 5)
        .with_tribe(Tribe::Beast)
        .on_deathrattle(on_deathrattle)
        .on_player_combat_summon(summoned_beast_bonus)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let bonus = if dying.is_golden { 14 } else { 7 };
    ctx.auras.add_effect(ID, bonus as u32, EffectDuration::Turn);
    for idx in 0..ctx.board.len() {
        if ctx.board[idx].tribe.matches(Tribe::Beast) {
            ctx.buff_unit(idx, bonus, bonus, "Goldrinn, the Great Wolf");
        }
    }
}

/// Until next turn, Beasts summoned in combat get the bonus too (the player effect's stacks).
fn summoned_beast_bonus(effect: &PlayerEffect, summoned: &mut Unit) {
    if summoned.tribe.matches(Tribe::Beast) {
        let bonus = effect.stacks as i32;
        summoned.add_stats(bonus, bonus);
    }
}
