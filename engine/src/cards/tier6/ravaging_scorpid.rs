//! `Ravaging Scorpid` (`BG36_209`) — Tier 6 Beast (`6/7`).
//!
//! After a friendly minion attacks, your Beetles have `+4/+4` (`+8/+8` if Golden) this game.
//! Deathrattle: Summon a (`2` if Golden) `2/2` Beetle.

use crate::cards::{tokens, BoardCtx, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit, UnitId};

pub const ID: CardId = 618;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Ravaging Scorpid", 6, 7, 6)
        .with_tribe(Tribe::Beast)
        .on_deathrattle(on_deathrattle)
        .on_friendly_attack(on_friendly_attack)
}

pub fn on_friendly_attack(ctx: &mut BoardCtx<'_>, self_idx: usize, _attacker_id: UnitId) {
    let bonus = 4 * ctx.board[self_idx].golden_mult();
    ctx.auras.beetle_bonus_atk += bonus;
    ctx.auras.beetle_bonus_hp += bonus;
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let beetle = tokens::make_beetle(false, ctx.auras);
        ctx.summon(dying.id, beetle);
    }
}
