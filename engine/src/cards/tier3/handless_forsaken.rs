//! `Handless Forsaken` (`BG25_010`) — Tier 3 Undead (`2/1`).
//! **Deathrattle:** Summon a (`two` if Golden) `2/1` Hand (`Helping Hand`) with **Reborn**.

use crate::cards::{tokens, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 319;
pub const NAME: &str = "Handless Forsaken";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 1, 3).with_tribe(Tribe::Undead)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let hand = tokens::make_helping_hand(false, ctx.auras);
        ctx.summon(dying.id, hand);
    }
}
