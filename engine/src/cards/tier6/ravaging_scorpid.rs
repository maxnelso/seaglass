//! `Ravaging Scorpid` (`BG36_209`) — Tier 6 Beast (`6/7`).
//!
//! After a friendly minion attacks, your Beetles have `+4/+4` (`+8/+8` if Golden) this game.
//! Deathrattle: Summon a (`2` if Golden) `2/2` Beetle.

use crate::cards::{tokens, CardTemplate, DeathrattleContext};
use crate::model::{CardId, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 618;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Ravaging Scorpid", 6, 7, 6).with_tribe(Tribe::Beast)
}

pub fn on_friendly_attack(board: &[Unit], auras: &mut PlayerAuras) {
    let mut bonus = 0i32;
    for u in board {
        if u.health > 0 && u.card_id == ID {
            bonus += if u.is_golden { 8 } else { 4 };
        }
    }
    if bonus > 0 {
        auras.beetle_bonus_atk += bonus;
        auras.beetle_bonus_hp += bonus;
    }
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let beetle = tokens::make_beetle(false, ctx.auras);
        ctx.summon(dying.id, beetle);
    }
}
